use crate::dht::{
    msg::{msg::Method, msg::TxId, Message},
    rpc::RpcCall,
    suspicious_node_detector::SuspiciousNodeDetector,
};
use crate::{
    cryptobox::Nonce,
    errors::{CryptoError, Error, NetworkError, ProtocolError, Result, StateError},
    CryptoBox, CryptoIdentity, EasyHandler, Id, Identity, LocalBoxHandler,
    LocalBoxTimerClient as TimerClient, NodeInfo,
};
use log::{debug, error, info, trace, warn};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    net::{SocketAddr, UdpSocket as StdUdpSocket},
    rc::Rc,
    sync::Arc,
    time::SystemTime,
};
use tokio::net::UdpSocket;

#[allow(dead_code)]
pub(crate) struct RpcServer {
    identity: Arc<CryptoIdentity>,
    ni: NodeInfo,

    suspicious_node_detector: Option<Rc<RefCell<dyn SuspiciousNodeDetector>>>,
    pending_calls: RefCell<HashMap<TxId, Rc<RefCell<RpcCall>>>>,

    recv_packets: RefCell<u32>,
    recv_packets_at_last_reachable_check: RefCell<u32>,
    last_reachable_check: RefCell<SystemTime>,
    is_reachable: RefCell<bool>,

    reachable_handler: RefCell<Option<LocalBoxHandler<bool>>>,
    message_handler: RefCell<Option<LocalBoxHandler<Rc<Message>>>>,
    callsent_handler: RefCell<Option<EasyHandler<Id>>>,
    calltimeout_handler: RefCell<Option<EasyHandler<Id>>>,

    start_time: RefCell<Option<SystemTime>>,
    is_running: RefCell<bool>,

    timer_client: Rc<TimerClient>,
    reachable_check_task: RefCell<Option<u64>>,

    tx_socket: RefCell<Option<Rc<StdUdpSocket>>>,
    rx_socket: RefCell<Option<Rc<StdUdpSocket>>>,
}

impl RpcServer {
    const MAX_ACTIVE_CALLS: usize = 64;
    const REACHABILITY_CHECK_INTERVAL: u64 = 5_000;
    const REACHABILITY_TIMEOUT: u64 = 60_000;

    pub(crate) fn new(
        ni: NodeInfo,
        identity: Arc<CryptoIdentity>,
        timer_client: Rc<TimerClient>,
        suspicious_node_detector: Option<Rc<RefCell<dyn SuspiciousNodeDetector>>>,
    ) -> Rc<Self> {
        Rc::new(Self {
            ni,
            identity,
            suspicious_node_detector,
            pending_calls: RefCell::new(HashMap::new()),
            recv_packets: RefCell::new(0),
            recv_packets_at_last_reachable_check: RefCell::new(0),
            last_reachable_check: RefCell::new(SystemTime::now()),
            is_reachable: RefCell::new(false),
            reachable_handler: RefCell::new(None),
            message_handler: RefCell::new(None),
            callsent_handler: RefCell::new(None),
            calltimeout_handler: RefCell::new(None),
            start_time: RefCell::new(None),
            is_running: RefCell::new(false),
            timer_client,
            reachable_check_task: RefCell::new(None),
            tx_socket: RefCell::new(None),
            rx_socket: RefCell::new(None),
        })
    }

    async fn check_reachability(self: &Rc<Self>) {
        let now = SystemTime::now();

        if *self.recv_packets.borrow() != *self.recv_packets_at_last_reachable_check.borrow() {
            self.set_reachable(true).await;
            *self.last_reachable_check.borrow_mut() = now;
            *self.recv_packets_at_last_reachable_check.borrow_mut() = *self.recv_packets.borrow();
            return;
        }

        if crate::elapsed_ms!(*self.last_reachable_check.borrow())
            > Self::REACHABILITY_TIMEOUT as u128
            && *self.recv_packets.borrow() != 0
            && *self.recv_packets_at_last_reachable_check.borrow() != 0
        {
            self.set_reachable(false).await;
        }
    }

    pub(crate) async fn set_reachable(self: &Rc<Self>, reachable: bool) {
        if *self.is_reachable.borrow() == reachable {
            return;
        }
        *self.is_reachable.borrow_mut() = reachable;
        let handler = self.reachable_handler.borrow_mut().take();
        if let Some(h) = handler {
            h.cb(reachable).await;
            *self.reachable_handler.borrow_mut() = Some(h);
        }
    }

    pub(crate) fn reachable_handler(self: &Rc<Self>, consumer: LocalBoxHandler<bool>) {
        *self.reachable_handler.borrow_mut() = Some(consumer);
    }

    pub(crate) fn is_reachable(self: &Rc<Self>) -> bool {
        *self.is_reachable.borrow()
    }

    pub(crate) fn has_pending_calls(self: &Rc<Self>) -> bool {
        !self.pending_calls.borrow().is_empty()
    }

    pub(crate) fn message_handler(self: &Rc<Self>, consumer: LocalBoxHandler<Rc<Message>>) {
        *self.message_handler.borrow_mut() = Some(consumer);
    }

    pub(crate) fn callsent_handler(self: &Rc<Self>, consumer: EasyHandler<Id>) {
        *self.callsent_handler.borrow_mut() = Some(consumer);
    }

    pub(crate) fn calltimeout_handler(self: &Rc<Self>, consumer: EasyHandler<Id>) {
        *self.calltimeout_handler.borrow_mut() = Some(consumer);
    }

    pub(crate) fn rx_tokio_socket(self: &Rc<Self>) -> Result<UdpSocket> {
        let socket = self.rx_socket.borrow().clone();
        let std_socket = socket
            .as_ref()
            .ok_or_else(|| -> Error { NetworkError::new("RPC server socket not initialized") })?
            .as_ref()
            .try_clone()
            .map_err(|e| NetworkError::new(format!("Failed to clone UDP socket: {e}")) as Error)?;

        std_socket.set_nonblocking(true).map_err(|e| {
            NetworkError::new(format!("Failed to configure UDP socket: {e}")) as Error
        })?;
        UdpSocket::from_std(std_socket).map_err(|e| -> Error {
            NetworkError::new(format!(
                "Failed to create Tokio UdpSocket from std UdpSocket: {e}"
            ))
        })
    }

    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        let socket_addr = self.ni.address();
        let socket = StdUdpSocket::bind(socket_addr).map_err(|e| {
            error!(
                "Rpc server failed to bind udp socket at {}: {e}",
                socket_addr
            );
            NetworkError::new(format!("{e}"))
        })?;
        let socket = Rc::new(socket);
        *self.rx_socket.borrow_mut() = Some(socket.clone());
        *self.tx_socket.borrow_mut() = Some(socket.clone());

        Ok(())
    }

    pub(crate) fn prepare(self: &Rc<Self>) -> bool {
        let now = SystemTime::now();
        *self.start_time.borrow_mut() = Some(now);
        *self.is_running.borrow_mut() = true;

        *self.is_reachable.borrow_mut() = true;
        *self.last_reachable_check.borrow_mut() = now;

        let server = self.clone();
        let result = self.timer_client.add_timer(
            Self::REACHABILITY_CHECK_INTERVAL,
            Some(Self::REACHABILITY_CHECK_INTERVAL),
            LocalBoxHandler::new(move |_| {
                let server = server.clone();
                Box::pin(async move {
                    server.check_reachability().await;
                })
            }),
        );

        let Ok(timer_id) = result else {
            error!("Failed to set reachability check timer.");
            return false;
        };
        *self.reachable_check_task.borrow_mut() = Some(timer_id);
        true
    }

    pub(crate) async fn stop(self: &Rc<Self>) {
        *self.reachable_handler.borrow_mut() = None;
        if !*self.is_running.borrow() {
            return;
        }

        if let Some(timer_id) = self.reachable_check_task.borrow_mut().take() {
            if let Err(e) = self.timer_client.cancel_timer(timer_id) {
                warn!("Failed to cancel reachability check timer: {e}");
            }
        }

        self.pending_calls.borrow_mut().clear();

        *self.tx_socket.borrow_mut() = None;
        *self.rx_socket.borrow_mut() = None;
        *self.start_time.borrow_mut() = None;
        *self.is_running.borrow_mut() = false;

        *self.is_reachable.borrow_mut() = false;

        info!("RPC server stopped at {}", self.ni.address());
    }

    pub(crate) fn send_call(self: &Rc<Self>, mut call: RpcCall) -> Result<()> {
        if self.pending_calls.borrow().len() >= Self::MAX_ACTIVE_CALLS {
            call.fail();
            return Err(StateError::new(format!(
                "Maximum number of active RPC calls ({}) reached",
                Self::MAX_ACTIVE_CALLS
            )));
        }

        let txid = call.txid();
        let target_id = call.target_id();
        let rs = self.clone();

        let handler = EasyHandler::new(move |_| {
            let existing = rs.pending_calls.borrow_mut().remove(&txid);
            if existing.is_none() {
                return;
            }

            let handler = rs.calltimeout_handler.borrow_mut().take();
            if let Some(h) = handler {
                h.cb(&target_id);
                *rs.calltimeout_handler.borrow_mut() = Some(h);
            }
        });

        let call = Rc::new(RefCell::new(call));
        call.borrow_mut().set_cloned(Rc::downgrade(&call));
        call.borrow_mut().set_timeout_handler(handler);
        call.borrow_mut()
            .set_timer_client(self.timer_client.clone());

        let mut msg = call.borrow_mut().take_transient();
        msg.set_nodeid(self.identity.id().clone());
        msg.set_associated_call(call.clone());

        self.pending_calls.borrow_mut().insert(txid, call.clone());

        let msg = Rc::new(msg);
        call.borrow_mut().set_request(msg.clone());

        match self.send_msg(&msg) {
            Ok(_) => {
                call.borrow_mut().sent();
                let handler = self.callsent_handler.borrow_mut().take();
                if let Some(h) = handler {
                    let target_id = call.borrow().target_id();
                    h.cb(&target_id);
                    *self.callsent_handler.borrow_mut() = Some(h);
                }
            }
            Err(e) => {
                let _ = self.pending_calls.borrow_mut().remove(&txid);
                call.borrow_mut().fail();
                return Err(e);
            }
        }
        Ok(())
    }

    pub(crate) fn send_msg(self: &Rc<Self>, msg: &Message) -> Result<usize> {
        // Deserialize message to bytes
        let data = serde_cbor::to_vec(msg).map_err(|e| -> Error {
            ProtocolError::new(format!("Failed to serialize message: {e}"))
        })?;

        // Encrypt message data with remote node's ID
        let cipher_len = CryptoBox::MAC_BYTES + Nonce::BYTES + data.len();
        let mut buf = vec![0u8; cipher_len + Id::BYTES];
        buf[..Id::BYTES].copy_from_slice(msg.nodeid().as_bytes());

        let rc = self
            .identity
            .encrypt(msg.remote_id(), &data, &mut buf[Id::BYTES..]);
        let encrypted = match rc {
            Ok(len) => len,
            Err(e) => return Err(CryptoError::new(format!("Failed to encrypt message: {e}"))),
        };
        if encrypted != cipher_len {
            return Err(CryptoError::new(format!(
                "Error: encrypted length {} does not match expected {}",
                encrypted, cipher_len
            )));
        }

        // Send message to remote node
        let socket = self.tx_socket.borrow().clone();
        let tx = socket
            .as_ref()
            .ok_or_else(|| -> Error { NetworkError::new("RPC server socket not initialized") })?;
        let sent_len = tx
            .send_to(&buf[..cipher_len + Id::BYTES], msg.remote_addr())
            .map_err(|e| -> Error { NetworkError::new(format!("Failed to send message: {e}")) })?;

        if sent_len != buf.len() {
            return Err(NetworkError::new(format!(
                "Error: sent length {} does not match expected {}",
                sent_len,
                buf.len()
            )));
        }

        if msg.method() == Method::Ping {
            trace!(
                "Message {}_{} to {}@{} was sent: {}",
                msg.method(),
                msg.kind(),
                msg.remote_id(),
                msg.remote_addr(),
                msg
            );
        } else {
            debug!(
                "Message {}_{} to {}@{} was sent: {}",
                msg.method(),
                msg.kind(),
                msg.remote_id(),
                msg.remote_addr(),
                msg
            );
        }

        Ok(sent_len)
    }

    #[inline]
    fn malformed_message(self: &Rc<Self>, from: SocketAddr) {
        if let Some(detector) = &self.suspicious_node_detector {
            detector.borrow_mut().malformed_message(from);
        }
    }

    #[inline]
    fn observe_message(self: &Rc<Self>, from: SocketAddr, id: Id) {
        if let Some(detector) = &self.suspicious_node_detector {
            detector.borrow_mut().observe(from, id);
        }
    }

    #[inline]
    fn inconsistent_socket(self: &Rc<Self>, from: SocketAddr, id: Id) {
        if let Some(detector) = &self.suspicious_node_detector {
            detector.borrow_mut().inconsistent(from, Some(id));
        }
    }

    pub(crate) async fn handle_packet(server: Rc<Self>, data: &[u8], from: SocketAddr) {
        let minimal_len = Id::BYTES + Nonce::BYTES + CryptoBox::MAC_BYTES + Message::MIN_BYTES;
        if data.len() < minimal_len {
            warn!("Ignored invalid packet from {}: too short", from);
            server.malformed_message(from);
            return;
        }

        // Decrypting remote node ID
        let from_id = match Id::try_from(&data[0..Id::BYTES]) {
            Ok(id) => id,
            Err(e) => {
                warn!("Ignored invalid packet from {}: {e}", from);
                server.malformed_message(from);
                return;
            }
        };

        // Decrypting message data.
        let identity = server.identity.clone();
        let decrypted = match identity.decrypt_into(&from_id, &data[Id::BYTES..]) {
            Ok(d) => d,
            Err(e) => {
                warn!("Ignored invalid packet from {}: {e}", from);
                server.malformed_message(from);
                return;
            }
        };

        // Deserializing message
        let mut msg = match serde_cbor::from_slice::<Message>(&decrypted) {
            Ok(m) => m,
            Err(e) => {
                warn!("Ignored invalid packet from {}: {e}", from);
                server.malformed_message(from);
                return;
            }
        };
        msg.set_nodeid(from_id);
        msg.set_remote(from_id, from);

        let mut recv_packets = server.recv_packets.borrow_mut();
        *recv_packets = recv_packets.wrapping_add(1);
        drop(recv_packets);

        debug!(
            "Received message {}_{} from {}@{}: {}",
            msg.method(),
            msg.kind(),
            from_id,
            from,
            msg
        );

        // Handle request message.
        let is_req = msg.is_req();
        if is_req {
            let handler = server.message_handler.borrow_mut().take();
            if let Some(handler) = handler {
                handler.cb(Rc::new(msg)).await;
                *server.message_handler.borrow_mut() = Some(handler);
            }
            return;
        }

        // Handle response or error message, matching with pending call.
        let msg_id = msg.txid();
        let call_opt = server.pending_calls.borrow_mut().remove(&msg_id);
        let Some(call) = call_opt else {
            server.observe_message(from, from_id);

            warn!(
                "Can not find RPC call for {} with txid {}, discard the message",
                msg.method(),
                msg.txid()
            );
            return;
        };
        let msg = Rc::new({
            msg.set_associated_call(call.clone());
            msg
        });

        {
            let mut locked = call.borrow_mut();
            let req = locked.req();
            if req.remote_addr() != &from || locked.target_id() != from_id {
                // Handle inconsistent socket (e.g., NAT issues or attack)
                // - the message is not a request
                // - the transaction ID matched
                // - response source or identity did not match request destination
                // this happening by chance is exceedingly unlikely indicates either port-mangling NAT,
                // a multihomed host listening on any-local address or some kind of attack
                let target_id = locked.target().id();
                warn!("Node address does not be consistent, ignored. request: {}@{} <- response: {}@{}",
                    target_id, req.remote_addr(), from_id, from);

                server.inconsistent_socket(from, from_id);
                // but expect an upcoming timeout if it's really just a misbehaving node
                locked.respond_inconsistent_socket();
                return;
            }

            // Checking message with same address but different method,
            // which is a strong signal of attack or misbehaving node.
            if msg.method() != req.method() {
                warn!(
                    "Got response with wrong method {} from {}@{} for {}",
                    msg.method(),
                    from_id,
                    from,
                    req.method()
                );

                locked.respond_wrong_method();
                server.malformed_message(from);
                drop(locked);
                return;
            }

            locked.respond(msg.clone());
        }

        let handler = server.message_handler.borrow_mut().take();
        if let Some(handler) = handler {
            handler.cb(msg).await;
            *server.message_handler.borrow_mut() = Some(handler);
        };
    }
}

impl fmt::Display for RpcServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RPC Server[{}]: {}@{}:{}",
            self.ni.default_family(),
            self.ni.id(),
            self.ni.host(),
            self.ni.port()
        )
    }
}
