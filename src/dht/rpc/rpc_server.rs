use std::{
    collections::HashMap,
    fmt,
    net::{SocketAddr, UdpSocket as StdUdpSocket},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime}
};
use log::{info, warn, error, debug};
use tokio::{
    net::UdpSocket,
    task::JoinHandle,
};

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
        StateError
    },
    dht::{
        suspicious_node_detector::SuspiciousNodeDetector,
        consumer::Consumer,
        rpc::RpcCall,
        msg::Message,
    }
};

#[allow(dead_code)]
pub(crate) struct RpcServer {
    identity            : Arc<CryptoIdentity>,
    ni                  : Arc<NodeInfo>,

    suspicious_node_detector: Option<Arc<Mutex<dyn SuspiciousNodeDetector>>>,
    pending_calls       : HashMap<i32, Arc<Mutex<RpcCall>>>,

    recv_packets        : u32,
    recv_packets_at_last_reachable_check: u32,
    last_reachable_check: SystemTime,
    is_reachable        : bool,

    reachable_handler   : Option<Consumer<bool>>,
    message_handler     : Option<Box<dyn Fn(&Message) + Send>>,
    callsent_handler    : Option<Box<dyn Fn(&mut RpcCall) + Send>>,
    calltimeout_handler : Option<Box<dyn Fn(&mut RpcCall) + Send>>,

    start_time          : Option<SystemTime>,
    is_running          : bool,
    reachable_check_task: Option<JoinHandle<()>>,

    tx_socket           : Option<Arc<StdUdpSocket>>,
    rx_socket           : Option<Arc<StdUdpSocket>>,
}

#[allow(dead_code)]
impl RpcServer {
    const MAX_ACTIVE_CALLS: usize = 64;
    pub(crate) const RPC_CALL_TIMEOUT_MAX: u64 = 10 * 1000;
    const REACHABILITY_CHECK_INTERVAL: Duration = Duration::from_millis(5_000);
    const REACHABILITY_TIMEOUT: Duration = Duration::from_millis(60_000);

    pub(crate) fn new(
        ni: Arc<NodeInfo>,
        identity: Arc<CryptoIdentity>,
        suspicious_node_detector: Option<Arc<Mutex<dyn SuspiciousNodeDetector>>>,
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
            reachable_check_task: None,

            tx_socket           : None,
            rx_socket           : None,
        }
    }

    fn check_reachability(&mut self) {
        let now = SystemTime::now();

        if self.recv_packets != self.recv_packets_at_last_reachable_check {
            self.set_reachable(true);
            self.last_reachable_check = now;
            self.recv_packets_at_last_reachable_check = self.recv_packets;
            return;
        }

        let timed_out = now
            .duration_since(self.last_reachable_check)
            .unwrap_or(Duration::ZERO) > Self::REACHABILITY_TIMEOUT;
        if timed_out && self.recv_packets != 0 && self.recv_packets_at_last_reachable_check != 0 {
            self.set_reachable(false);
            // TODO: reset timeout_sampler
        }
    }

    pub(crate) fn set_reachable(&mut self, reachable: bool) {
        if self.is_reachable == reachable {
            return;
        }
        self.is_reachable = reachable;
        if let Some(handler) = self.reachable_handler.as_ref() {
            handler.accept(reachable);
        }
    }

    pub(crate) fn reachable_handler<F>(&mut self, cb: F)
    where F: Fn(bool) + Send + 'static,
    {
        self.reachable_handler = Some(Consumer::new(cb));
    }

    pub(crate) fn is_reachable(&self) -> bool {
        self.is_reachable
    }

    pub(crate) fn has_pending_calls(&self) -> bool {
        !self.pending_calls.is_empty()
    }

    pub(crate) fn age(&self) -> Duration {
        self.start_time
            .and_then(|start_time| start_time.elapsed().ok())
            .unwrap_or(Duration::ZERO)
    }

    pub(crate) fn message_handler<F>(&mut self, cb: F)
    where F: Fn(&Message) + Send + 'static,
    {
        self.message_handler = Some(Box::new(cb));
    }

    pub(crate) fn callsent_handler<F>(&mut self, cb: F)
    where F: Fn(&mut RpcCall) + Send + 'static,
    {
        self.callsent_handler = Some(Box::new(cb));
    }

    pub(crate) fn calltimeout_handler<F>(&mut self, cb: F)
    where F: Fn(&mut RpcCall) + Send + 'static,
    {
        self.calltimeout_handler = Some(Box::new(cb));
    }

    /*
    pub(crate) fn start_reachability_check(server: Arc<Mutex<RpcServer>>) {
        let task_server = server.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Self::REACHABILITY_CHECK_INTERVAL.mul_f32(2.0)).await;

            loop {
                {
                    let mut locked = match task_server.lock() {
                        Ok(locked) => locked,
                        Err(_) => break,
                    };

                    if !locked.is_running {
                        break;
                    }

                    locked.check_reachability();
                }

                tokio::time::sleep(Self::REACHABILITY_CHECK_INTERVAL).await;
            }
        });

        if let Ok(mut locked) = server.lock() {
            if let Some(previous) = locked.reachable_check_task.replace(task) {
                previous.abort();
            }
        }
    }
    */

    pub(crate) async fn start(&mut self) -> Result<()> {
        let socket_addr = self.ni.socket_addr();
        //let socket = match UdpSocket::bind(socket_addr).await {
        let socket = match StdUdpSocket::bind(socket_addr) {
            Ok(socket) => Arc::new(socket),
            Err(e) => {
                error!("Rpc server failed to bind udp socket at {}: {e}", socket_addr);
                return Err(NetworkError::new(format!("{e}")));
            }
        };
        self.rx_socket = Some(socket.clone());
        self.tx_socket = Some(socket.clone());

        let now = SystemTime::now();
        self.start_time     = Some(now);
        self.is_running     = true;
        self.is_reachable   = true;
        self.last_reachable_check = now;
        self.recv_packets   = 0;
        self.recv_packets_at_last_reachable_check = 0;

        Ok(())
    }

    pub(crate) async fn stop(&mut self) {
        self.reachable_handler = None;
        if !self.is_running {
            return;
        }

        self.pending_calls.clear();

        if let Some(task) = self.reachable_check_task.take() {
            task.abort();
        }

        self.tx_socket  = None;
        self.rx_socket  = None;
        self.start_time = None;
        self.is_running = false;
        self.is_reachable = false;

        info!("RPC server stopped at {}", self.ni.socket_addr());
    }

    pub(crate) fn send_call(&mut self, mut call: RpcCall) -> Result<()>{
        if self.pending_calls.len() >= Self::MAX_ACTIVE_CALLS {
            return Err(StateError::new("Too many active calls pending in the queue."));
        }

        let txid = call.txid();
        let mut msg  = call.take_transient();
        msg.set_nodeid(*self.ni.id());

        let call = Arc::new(Mutex::new(call));
        msg.set_associated_call(call.clone());
        self.pending_calls.insert(txid, call.clone());

        let mut locked = call.lock().unwrap();

        let msg = Arc::new(msg);
        locked.set_request(msg.clone());

        match self.send_msg(msg.as_ref()) {
            Ok(_) => {
                locked.sent();
                if let Some(handler) = self.callsent_handler.as_ref() {
                    handler(&mut *locked);
                }
            },
            Err(e) => {
                let _ = self.pending_calls.remove(&txid);
                locked.fail(&e);
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
            Err(e) => return Err(NetworkError::new(format!("Failed to send message: {e}")))
        };
        if sent_len != buf.len() {
            return Err(NetworkError::new(format!("Error: sent length {} does not match expected {}", sent_len, buf.len())) as crate::errors::Error);
        }

        debug!("Message <{}@{} to {}@{}> was sent : {}",
            msg.method(), msg.kind(), msg.remote_id(), msg.remote_addr(), msg);

        Ok(sent_len)
    }

    #[inline]
    fn malformed_message(&self, from: SocketAddr) {
        self.suspicious_node_detector.as_ref().map(|v| {
            v.lock().unwrap().malformed_message(from);
        });
    }

    #[inline]
    fn observe_message(&self, from: SocketAddr, id: Id) {
        self.suspicious_node_detector.as_ref().map(|v| {
            v.lock().unwrap().observe(from, id);
        });
    }

    #[inline]
    fn inconsistent_socket(&self, from: SocketAddr, id: Id) {
        self.suspicious_node_detector.as_ref().map(|v| {
            v.lock().unwrap().inconsistent(from, Some(id));
        });
    }
}

fn handle_packet(server: &Arc<Mutex<RpcServer>>, data: &[u8], from: SocketAddr) {
    let malformed_message = |from: SocketAddr| {
        server.lock().unwrap().malformed_message(from);
    };
    let observe_message = |from: SocketAddr, id: Id| {
        server.lock().unwrap().observe_message(from, id);
    };
    let inconsistent_socket = |from: SocketAddr, id: Id| {
        server.lock().unwrap().inconsistent_socket(from, id);
    };

    let minimal_len = Id::BYTES + CryptoBox::MAC_BYTES + Message::MIN_BYTES;
    if data.len() < minimal_len {
        warn!("Ignored invalid packet from {}: too short", from);
        malformed_message(from);
        return;
    }

    // Decrypting remote node ID
    let rc = Id::try_from(&data[0.. Id::BYTES]);
    if let Err(e) = rc.as_ref() {
        warn!("Ignored invalid packet from {}: invalid nodeid {e}", from);
        malformed_message(from);
        return;
    }

    let from_id = rc.unwrap();

    // TODO: blacklist checking.

    // Decrypting message data.
    let identity = server.lock().unwrap().identity.clone();
    let rc = identity.decrypt_into(&from_id, &data[Id::BYTES ..]);
    if let Err(e) = rc.as_ref() {
        warn!("Ignored invalid packet from {}: decrypting error {e}", from);
        malformed_message(from);
        return;
    };
    let decrypted = rc.unwrap();

    // Deserializing message
    let rc = serde_cbor::from_slice::<Message>(&decrypted);
    if let Err(e) = rc.as_ref() {
        warn!("Ignored invalid packet from {}: deserializing error {e}", from);
        malformed_message(from);
        return;
    }

    // Assembling message.
    let mut msg = rc.unwrap();
    msg.set_nodeid(from_id);
    msg.set_remote(from_id, from);

    debug!("Received message <{}-{} from {}@{}>: {}",
        msg.method(), msg.kind(), from_id, from, msg);

    // Handle request message.
    if msg.is_req() {
        let message_cb = server.lock().unwrap().message_handler.take();
        if let Some(cb) = message_cb {
            cb(&msg);
            server.lock().unwrap().message_handler = Some(cb);
        }
        return;
    }

    // Handle response or error message, matching with pending call.
    let msg_id = msg.txid();
    let call_opt = server.lock().unwrap().pending_calls.remove(&msg_id);
    let Some(call) = call_opt else {
        observe_message(from, from_id);

        warn!("Can not find RPC call for {} with txid {}, discard the message",
            msg.method(), msg.txid());
        return;
    };
    let msg = Arc::new({
        msg.set_associated_call(call.clone());
        msg
    });

    let mut locked = call.lock().unwrap();
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

        inconsistent_socket(from, from_id);
        // but expect an upcoming timeout if it's really just a misbehaving node
        locked.respond_inconsistent_socket(msg);
        return;
    }

    // Checking message with same address but different method,
    // which is a strong signal of attack or misbehaving node.
    if msg.method() != req.method() {
        warn!("Got response with wrong method {} from {}@{} for {}",
            msg.method(), from_id, from, req.method());

        locked.respond_wrong_method(msg);
        malformed_message(from);
        return;
    }

    locked.respond(msg.clone());
    drop(locked);

    let message_cb = server.lock().unwrap().message_handler.take();
    if let Some(cb) = message_cb {
        cb(msg.as_ref());
        server.lock().unwrap().message_handler = Some(cb);
    };

    // TODO: handle metrics.
}

pub(crate) async fn run_loop(
    server: Arc<Mutex<RpcServer>>,
    quit_flag: Arc<Mutex<bool>>,
) -> bool {
    let socket = server.lock().unwrap().rx_socket.take().unwrap();

    let Ok(std_socket) = socket.try_clone() else {
        return false;
    };
    let Ok(_) = std_socket.set_nonblocking(true) else {
        return false;
    };
    let Ok(async_socket) = UdpSocket::from_std(std_socket) else {
        return false;
    };

    info!("RPC server started at {}", server.lock().unwrap().ni.socket_addr());
    let mut buf = vec![0u8; 2048];
    loop {
        let packet = async_socket.recv_from(&mut buf).await;
        if let Err(e) = packet.as_ref() {
            error!("Rpc server failed to receive packet: {e}");
            if *quit_flag.lock().unwrap() {
                break;
            } else {
                continue;
            }
        }
        let (len, from) = packet.unwrap();
        handle_packet(&server, &buf[..len], from);
    };
    true
}

impl fmt::Display for RpcServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RPC Server[{}]: {}@{}:{}, uptime: {:?}",
            self.ni.network(),
            self.ni.id(),
            self.ni.host(),
            self.ni.port(),
            self.age()
        )
    }
}
