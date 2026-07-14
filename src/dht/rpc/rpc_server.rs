use std::{
    fmt,
    rc::{Rc, Weak},
    sync::Arc,
    cell::RefCell,
    collections::HashMap,
    time::SystemTime,
    net::{SocketAddr, UdpSocket as StdUdpSocket},
};
use log::{info, warn, error, debug};
use tokio::net::UdpSocket;
use crate::{
    CryptoBox,
    CryptoIdentity,
    Id, Identity,
    NodeInfo,
    cryptobox::Nonce,

    errors::{
        Error,
        Result,
        CryptoError,
        NetworkError,
        ProtocolError,
    }
};
use crate::dht::{
    timer_client::LocalTimerClient as TimerClient,
    suspicious_node_detector::SuspiciousNodeDetector,
    handler::{Handler, LocalHandler as AsyncHandler},
    rpc::RpcCall,
    msg::Message
};

#[allow(dead_code)]
pub(crate) struct RpcServer {
    identity            : Arc<CryptoIdentity>,
    ni                  : NodeInfo,

    suspicious_node_detector: Option<Rc<RefCell<dyn SuspiciousNodeDetector>>>,
    pending_calls       : HashMap<i32, Rc<RefCell<RpcCall>>>,

    recv_packets        : u32,
    recv_packets_at_last_reachable_check: u32,
    last_reachable_check: SystemTime,
    is_reachable        : bool,

    reachable_handler   : Option<AsyncHandler<bool>>,
    message_handler     : Option<AsyncHandler<Rc<Message>>>,
    callsent_handler    : Option<Handler<RpcCall>>,
    calltimeout_handler : Option<Handler<RpcCall>>,

    start_time          : Option<SystemTime>,
    is_running          : bool,

    timer_client        : Rc<TimerClient>,
    reachable_check_task: Option<u64>,

    tx_socket           : Option<Rc<StdUdpSocket>>,
    rx_socket           : Option<Rc<StdUdpSocket>>,

    cloned              : Weak<RefCell<RpcServer>>,
}

impl RpcServer {
    const MAX_ACTIVE_CALLS: usize = 64;
    pub(crate) const RPC_CALL_TIMEOUT_MAX: u64 = 10 * 1000;
    const REACHABILITY_CHECK_INTERVAL   :u64 = 5_000;
    const REACHABILITY_TIMEOUT          :u64 = 60_000;

    pub(crate) fn new(
        ni: NodeInfo,
        identity: Arc<CryptoIdentity>,
        timer_client: Rc<TimerClient>,
        suspicious_node_detector: Option<Rc<RefCell<dyn SuspiciousNodeDetector>>>,
    ) -> Self {

        Self {
            ni,
            identity,
            suspicious_node_detector,
            pending_calls       : HashMap::new(),
            recv_packets        : 0,
            recv_packets_at_last_reachable_check: 0,
            last_reachable_check: SystemTime::now(),
            is_reachable        : false,
            reachable_handler   : None,
            message_handler     : None,
            callsent_handler    : None,
            calltimeout_handler : None,
            start_time          : None,
            is_running          : false,
            timer_client,
            reachable_check_task: None,

            tx_socket           : None,
            rx_socket           : None,

            cloned              : Weak::new(),
        }
    }

    pub(crate) fn set_cloned(&mut self, cloned: Weak<RefCell<RpcServer>>) {
        self.cloned = cloned;
    }

    async fn check_reachability(&mut self) {
        let now = SystemTime::now();

        if self.recv_packets != self.recv_packets_at_last_reachable_check {
            self.set_reachable(true).await;
            self.last_reachable_check = now;
            self.recv_packets_at_last_reachable_check = self.recv_packets;
            return;
        }

        if crate::elapsed_ms!(self.last_reachable_check) > Self::REACHABILITY_TIMEOUT as u128 &&
            self.recv_packets != 0 &&
            self.recv_packets_at_last_reachable_check != 0 {
            self.set_reachable(false).await;
        }
    }

    pub(crate) async fn set_reachable(&mut self, reachable: bool) {
        if self.is_reachable == reachable {
            return;
        }
        self.is_reachable = reachable;
        if let Some(handler) = self.reachable_handler.as_ref() {
            handler.cb(reachable).await;
        }
    }

    pub(crate) fn reachable_handler(&mut self, consumer: AsyncHandler<bool>) {
        self.reachable_handler = Some(consumer);
    }

    pub(crate) fn is_reachable(&self) -> bool {
        self.is_reachable
    }

    pub(crate) fn has_pending_calls(&self) -> bool {
        !self.pending_calls.is_empty()
    }

    pub(crate) fn message_handler(&mut self, consumer: AsyncHandler<Rc<Message>>) {
        self.message_handler = Some(consumer);
    }

    pub(crate) fn callsent_handler(&mut self, consumer: Handler<RpcCall>) {
        self.callsent_handler = Some(consumer);
    }

    pub(crate) fn calltimeout_handler(&mut self, consumer: Handler<RpcCall>) {
        self.calltimeout_handler = Some(consumer);
    }

    pub(crate) fn rx_tokio_socket(&self) -> Result<UdpSocket> {
        let std_socket = self.rx_socket.as_ref().ok_or_else(|| -> Error {
            NetworkError::new("RPC server socket not initialized")
        })?.as_ref().try_clone().map_err(|e| {
            NetworkError::new(format!("Failed to clone UDP socket: {e}")) as Error
        })?;

        std_socket.set_nonblocking(true).map_err(|e| {
            NetworkError::new(format!("Failed to configure UDP socket: {e}")) as Error
        })?;
        UdpSocket::from_std(std_socket).map_err(|e| -> Error {
            NetworkError::new(format!("Failed to create Tokio UdpSocket from std UdpSocket: {e}"))
        })
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        let socket_addr = self.ni.socket_addr();
        let socket = StdUdpSocket::bind(socket_addr).map_err(|e| {
            error!("Rpc server failed to bind udp socket at {}: {e}", socket_addr);
            NetworkError::new(format!("{e}"))
        })?;
        let socket = Rc::new(socket);
        self.rx_socket = Some(socket.clone());
        self.tx_socket = Some(socket.clone());

        Ok(())
    }

    pub(crate) fn prepare(&mut self) -> bool {
        let now = SystemTime::now();
        self.start_time = Some(now);
        self.is_running = true;

        self.is_reachable   = true;
        self.last_reachable_check = now;

        let cloned = self.cloned.upgrade().expect("RpcServer weak reference not set");
        let result = self.timer_client.add_timer(
            Self::REACHABILITY_CHECK_INTERVAL,
            Some(Self::REACHABILITY_CHECK_INTERVAL),
            AsyncHandler::new(move |_| {
                let server = cloned.clone();
                Box::pin(async move {
                    server.borrow_mut().check_reachability().await;
                })
            })
        );

        let Ok(timer_id) = result else {
            error!("Failed to set reachability check timer.");
            return false;
        };
        self.reachable_check_task = Some(timer_id);
        true
    }

    pub(crate) async fn stop(&mut self) {
        self.reachable_handler = None;
        if !self.is_running {
            return;
        }

        self.pending_calls.clear();

        self.tx_socket  = None;
        self.rx_socket  = None;
        self.start_time = None;
        self.is_running = false;

        self.is_reachable = false;
        self.reachable_check_task = None;

        info!("RPC server stopped at {}", self.ni.socket_addr());
    }

    pub(crate) fn send_call(&mut self, mut call: RpcCall) -> Result<()>{
        if self.pending_calls.len() >= Self::MAX_ACTIVE_CALLS {
       // return Err(StateError::new("Too many active calls pending in the queue.") as Box<dyn Error>);
            return Ok(());
        }

        let txid = call.txid();
        let mut msg  = call.take_transient();
        msg.set_nodeid(*self.ni.id());

        let call = Rc::new(RefCell::new(call));
        call.borrow_mut().set_cloned(Rc::downgrade(&call));
        msg.set_associated_call(call.clone());
        self.pending_calls.insert(txid, call.clone());

        let mut locked = call.borrow_mut();

        let msg = Rc::new(msg);
        locked.set_request(msg.clone());

        match self.send_msg(msg.as_ref()) {
            Ok(_) => {
                let timer_client = self.timer_client.clone();
                locked.sent(timer_client);
                if let Some(handler) = self.callsent_handler.as_ref() {
                    handler.cb(&locked);
                }
            },
            Err(e) => {
                let _ = self.pending_calls.remove(&txid);
                locked.fail();
                return Err(e);
            }
        }
        Ok(())
    }

    pub(crate) fn send_msg(&self, msg: &Message) -> Result<usize> {
        // Deserialize message to bytes
        let data = serde_cbor::to_vec(msg).map_err(|e| -> Error {
            ProtocolError::new(format!("Failed to serialize message: {e}"))
        })?;

        // Encrypt message data with remote node's ID
        let cipher_len = CryptoBox::MAC_BYTES + Nonce::BYTES + data.len();
        let mut buf = vec![0u8; cipher_len + Id::BYTES];
        buf[..Id::BYTES].copy_from_slice(msg.nodeid().as_bytes());

        let rc = self.identity.encrypt(
            msg.remote_id(), &data, &mut buf[Id::BYTES..]
        );
        let encrypted = match rc {
            Ok(len) => len,
            Err(e) => return Err(CryptoError::new(format!("Failed to encrypt message: {e}")))
        };
        if encrypted != cipher_len {
            return Err(CryptoError::new(format!("Error: encrypted length {} does not match expected {}",
                encrypted, cipher_len)));
        }

        // Send message to remote node
        let rc = self.tx_socket.as_ref().unwrap().send_to(
            &buf[..cipher_len + Id::BYTES], msg.remote_addr()
        );

        let sent_len = match rc {
            Ok(len) => len,
            Err(e) => return Err(NetworkError::new(
                                format!("Failed to send message: {e}")))
        };
        if sent_len != buf.len() {
            return Err(NetworkError::new(
                format!("Error: sent length {} does not match expected {}", sent_len, buf.len())));
        }

        debug!("Message <{}@{} to {}@{}> was sent : {}",
            msg.method(), msg.kind(), msg.remote_id(), msg.remote_addr(), msg);

        Ok(rc.unwrap())
    }

    #[inline]
    fn malformed_message(&self, from: SocketAddr) {
        self.suspicious_node_detector.as_ref().map(|v| {
            v.borrow_mut().malformed_message(from);
        });
    }

    #[inline]
    fn observe_message(&self, from: SocketAddr, id: Id) {
        self.suspicious_node_detector.as_ref().map(|v| {
            v.borrow_mut().observe(from, id);
        });
    }

    #[inline]
    fn inconsistent_socket(&self, from: SocketAddr, id: Id) {
        self.suspicious_node_detector.as_ref().map(|v| {
            v.borrow_mut().inconsistent(from, Some(id));
        });
    }

    pub(crate) async fn handle_packet(server: Rc<RefCell<Self>>, data: &[u8], from: SocketAddr) {
        let minimal_len = Id::BYTES + CryptoBox::MAC_BYTES + Message::MIN_BYTES;
        if data.len() < minimal_len {
            warn!("Ignored invalid packet from {}: too short", from);
            server.borrow().malformed_message(from);
            return;
        }

        // Decrypting remote node ID
        let rc = Id::try_from(&data[0.. Id::BYTES]);
        if let Err(e) = rc.as_ref() {
            warn!("Ignored invalid packet from {}: invalid nodeid {e}", from);
            server.borrow().malformed_message(from);
            return;
        }

        let from_id = rc.unwrap();

        // TODO: blacklist checking.

        // Decrypting message data.
        let identity = server.borrow().identity.clone();
        let rc = identity.decrypt_into(&from_id, &data[Id::BYTES ..]);
        if let Err(e) = rc.as_ref() {
            warn!("Ignored invalid packet from {}: decrypting error {e}", from);
            server.borrow().malformed_message(from);
            return;
        };
        let decrypted = rc.unwrap();

        // Deserializing message
        let rc = serde_cbor::from_slice::<Message>(&decrypted);
        if let Err(e) = rc.as_ref() {
            warn!("Ignored invalid packet from {}: deserializing error {e}", from);
            server.borrow().malformed_message(from);
            return;
        }

        // Assembling message.
        let mut msg = rc.unwrap();
        msg.set_nodeid(from_id);
        msg.set_remote(from_id, from);

        debug!("Received message <{}-{} from {}@{}>: {}",
            msg.method(), msg.kind(), from_id, from, msg);

        // Handle request message.
        let is_req = msg.is_req();
        if is_req {
            let handler = server.borrow_mut().message_handler.take();
            if let Some(handler) = handler {
                handler.cb(Rc::new(msg)).await;
                server.borrow_mut().message_handler = Some(handler);
            }
            return;
        }

        // Handle response or error message, matching with pending call.
        let msg_id = msg.txid();
        let call_opt = server.borrow_mut().pending_calls.remove(&msg_id);
        let Some(call) = call_opt else {
            server.borrow().observe_message(from, from_id);

            warn!("Can not find RPC call for {} with txid {}, discard the message",
                msg.method(), msg.txid());
            return;
        };
        let msg = Rc::new({
            msg.set_associated_call(call.clone());
            msg
        });

        {
            let mut locked = call.borrow_mut();
            let req = locked.req();
            if req.remote_addr() != &from {
                // Handle inconsistent socket (e.g., NAT issues or attack)
                // - the message is not a request
                // - the transaction ID matched
                // - response source did not match request destination
                // this happening by chance is exceedingly unlikely indicates either port-mangling NAT,
                // a multihomed host listening on any-local address or some kind of attack
                let target_id = locked.target().id();
                warn!("Node address does not be consistent, ignored. request: {}@{} <- response: {}@{}",
                    target_id, req.remote_addr(), from_id, from);

                server.borrow().inconsistent_socket(from, from_id);
                // but expect an upcoming timeout if it's really just a misbehaving node
                locked.respond_inconsistent_socket();
                return;
            }

            // Checking message with same address but different method,
            // which is a strong signal of attack or misbehaving node.
            if msg.method() != req.method() {
                warn!("Got response with wrong method {} from {}@{} for {}",
                    msg.method(), from_id, from, req.method());

                locked.respond_wrong_method();
                server.borrow().malformed_message(from);
                return;
            }

            locked.respond(msg.clone());
        }

        let handler = server.borrow_mut().message_handler.take();
        if let Some(handler) = handler {
            handler.cb(msg).await;
            server.borrow_mut().message_handler = Some(handler);
        };
        // TODO: handle metrics.
    }
}

impl fmt::Display for RpcServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RPC Server[{}]: {}@{}:{}",
            self.ni.network(),
            self.ni.id(),
            self.ni.host(),
            self.ni.port()
        )
    }
}
