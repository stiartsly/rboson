use std::{
    fmt,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
    collections::HashMap,
};
use log::{info, warn, error, trace};
use tokio_util::time::{delay_queue::Key, DelayQueue};
use tokio::{
    net::UdpSocket,
    task::JoinHandle,
    sync::oneshot,
    sync::mpsc::{self, UnboundedSender, UnboundedReceiver},
};

use crate::{
    CryptoBox, CryptoIdentity, Id, Identity, NodeInfo, cryptobox::Nonce, dht::suspicious_node_detector::SuspiciousNodeDetector, errors::{
        CryptoError, Error, NetworkError, ProtocolError, Result, StateError
    }
};
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    rpc::RpcCall,
    msg::Message,
    timer::{self, Job, Command},
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
    reachable           : bool,

    reachable_handler   : Option<Consumer<bool>>,
    message_handler     : Option<Box<dyn Fn(&Message) + Send>>,
    callsent_handler    : Option<Box<dyn Fn(&mut RpcCall) + Send>>,
    calltimeout_handler : Option<Box<dyn Fn(&mut RpcCall) + Send>>,

    start_time          : Option<SystemTime>,
    is_running          : bool,
    reachable_check_task: Option<JoinHandle<()>>,

    timer_client        : Option<Arc<timer::Client>>,
    tx_channel          : Option<UnboundedSender<Command>>,
    rx_channel          : Option<UnboundedReceiver<Command>>,

    tx_socket           : Option<Arc<UdpSocket>>,
    rx_socket           : Option<Arc<UdpSocket>>,
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
            reachable           : false,
            reachable_handler   : None,
            message_handler     : None,
            callsent_handler    : None,
            calltimeout_handler : None,
            start_time          : None,
            is_running          : false,
            reachable_check_task: None,

            timer_client        : None,
            tx_channel          : None,
            rx_channel          : None,
            tx_socket           : None,
            rx_socket           : None,
        }
    }

    fn identity(&self) -> Arc<CryptoIdentity> {
        self.identity.clone()
    }

    pub(crate) fn timer_client(&self) -> Arc<timer::Client> {
        self.timer_client.as_ref()
            .expect("timer client should be initialized")
            .clone()
    }

    fn tx_socket(&self) -> &Arc<UdpSocket> {
        self.tx_socket.as_ref().expect("socket should be initialized")
    }

    fn rx_channel_take(&mut self) -> UnboundedReceiver<Command> {
        self.rx_channel.take().expect("rx channel should be initialized")
    }

    fn rx_socket_take(&mut self) -> Arc<UdpSocket> {
        self.rx_socket.take().expect("socket should be initialized")
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
        if self.reachable == reachable {
            return;
        }
        self.reachable = reachable;
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
        self.reachable
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
        let (tx, rx) = mpsc::unbounded_channel::<Command>();
        self.timer_client = Some(Arc::new(timer::Client::new(tx.clone())));
        self.rx_channel = Some(rx);
        self.tx_channel = Some(tx);

        let socket_addr = self.ni.socket_addr();
        let socket = match UdpSocket::bind(socket_addr).await {
            Ok(socket) => Arc::new(socket),
            Err(e) => {
                error!("Rpc server failed to bind udp socket at {}: {e}", socket_addr);
                return Err(NetworkError::new(format!("{e}")));
            }
        };
        self.rx_socket = Some(socket.clone());
        self.tx_socket = Some(socket.clone());

        let now = SystemTime::now();
        self.start_time = Some(now);
        self.is_running = true;
        self.reachable = true;
        self.last_reachable_check = now;
        self.recv_packets = 0;
        self.recv_packets_at_last_reachable_check = 0;

        Ok(())
    }

    pub(crate) fn run_loop_context(
        rpc_server: Arc<Mutex<RpcServer>>,
        dht: Arc<Mutex<DHT>>,
    ) -> RunLoopContext {
        RunLoopContext::new(rpc_server, dht)
    }

    pub(crate) fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        // Signal run_loop to stop (fire-and-forget; don't wait for reply)
        let (reply_tx, _) = oneshot::channel();
        if let Some(tx) = self.tx_channel.as_ref() {
            let _ = tx.send(Command::Stop { reply: reply_tx });
        }

        self.pending_calls.clear();

        if let Some(task) = self.reachable_check_task.take() {
            task.abort();
        }

        self.tx_socket = None;
        self.rx_socket = None;
        self.tx_channel = None;
        self.rx_channel = None;

        self.is_running = false;

        self.reachable = false;
        self.start_time = None;

        info!("RPC server stopped at {}", self.ni.socket_addr());
    }

    pub(crate) async fn send_call(&mut self, call: RpcCall) -> Result<()>{
        if self.pending_calls.len() >= Self::MAX_ACTIVE_CALLS {
            return Err(StateError::new(format!("Too many active calls pending in the queue.")) as Error);
        }

        let txid = call.txid();
        let call = Arc::new(Mutex::new(call));
        self.pending_calls.insert(txid, call.clone());

        let mut locked = crate::locked!(call);
        let mut msg = locked.req_mut();
        match self.send_msg(&mut msg).await {
            Ok(_) => {
                locked.sent();
                if let Some(handler) = self.callsent_handler.as_ref() {
                    handler(&mut *locked);
                }
            },
            Err(e) => {
                let _ = self.pending_calls.remove(&txid);
                // locked.fail(e);
                return Err(e);
            }
        }
        Ok(())
    }

    pub(crate) async fn send_msg(&self, msg: &mut Message) -> Result<usize> {
        let nodeid = self.ni.id().clone();
        msg.set_nodeid(nodeid);

        let data = serde_cbor::to_vec(&msg).map_err(|e| -> Error {
            ProtocolError::new(format!("Failed to serialize message: {e}"))
        })?;
        let len = data.len() + Nonce::BYTES + CryptoBox::MAC_BYTES;

        let mut buf = vec![0u8; len + Id::BYTES];
        buf[..Id::BYTES].copy_from_slice(msg.nodeid().as_bytes());

        let encrypted_len = self.identity.encrypt(
            msg.remote_id(), &data, &mut buf[Id::BYTES..]
        ).map_err(|e| -> Error {
            CryptoError::new(format!("Failed to encrypt message: {e}"))
        })?;
        if encrypted_len != len {
            return Err(CryptoError::new(format!("Error: encrypted length {} does not match expected {}", encrypted_len, len)) as Error);
        }

        let sent_len = self.tx_socket().send_to(
            &buf[..len + Id::BYTES], msg.remote_addr()
        ).await.map_err(|e| -> crate::errors::Error {
            NetworkError::new(format!("Failed to send message: {e}"))
        })?;
        if sent_len != len + Id::BYTES {
            return Err(NetworkError::new(format!("Error: sent length {} does not match expected {}", sent_len, len + Id::BYTES)) as crate::errors::Error);
        }
        Ok(sent_len)
    }

    fn parse_packet(
        &mut self,
        data: &[u8],
        from: &SocketAddr
    ) -> Option<Message> {
        if data.len() < Id::BYTES + CryptoBox::MAC_BYTES + Message::MIN_BYTES {
            warn!("Ignored invalid packet from {}: too short", from);
            if let Some(detector) = self.suspicious_node_detector.as_ref() {
                detector.lock().unwrap().malformed_message(from.clone());
            }
            return None;
        }

        let from_id = match Id::try_from(&data[0.. Id::BYTES]) {
            Ok(v) => v,
            Err(e) => {
                warn!("Ignored invalid packet from {}: invalid nodeid {e}", from);
                if let Some(detector) = self.suspicious_node_detector.as_ref() {
                    detector.lock().unwrap().malformed_message(from.clone());
                }
                return None;
            }
        };

        // TOOD: blacklist.

        if let Some(detector) = self.suspicious_node_detector.as_ref() {
            if detector.lock().unwrap().is_banned(&from.ip()) {
                warn!("Ignored packet from suspicious node {}@{}", from_id, from);
                return None;
            }
        }

        let decrypted = match self.identity.decrypt_into(&from_id, &data[Id::BYTES ..]) {
            Ok(v) => v,
            Err(e) => {
                warn!("Ignored invalid packet from {}: decrypting error {e}", from);
                if let Some(detector) = self.suspicious_node_detector.as_ref() {
                    detector.lock().unwrap().malformed_message(from.clone());
                }
                return None;
            }
        };

        let mut msg = match serde_cbor::from_slice::<Message>(&decrypted) {
            Ok(mut msg) => {
                msg.set_nodeid(from_id);
                msg.set_remote(from_id, from.clone());
                msg
            },
            Err(e) => {
                warn!("Ignored invalid packet from {}: deserializing error {e}", from);
                if let Some(detector) = self.suspicious_node_detector.as_ref() {
                    detector.lock().unwrap().malformed_message(from.clone());
                }
                return None;
             }
        };

        trace!("Received {}:{} from {}@{}: {}", msg.method(), msg.kind(),
                from_id, from, msg);

        self.recv_packets += 1;

        // Handle request
        if msg.is_req() {
            return Some(msg);
        }

        // Handle response
        let call = self.pending_calls.get(&msg.txid()).cloned();
        let Some(call) = call else {
            if let Some(detector) = self.suspicious_node_detector.as_ref() {
                detector.lock().unwrap().observe(from.clone(), from_id);
            }

            warn!("Can not find RPC call for response {} with txid {}, discard the response",
                msg.method(), msg.txid());
            return None;
        };

        let mut locked = call.lock().unwrap();
        let req = locked.req();
        if req.remote_addr() == from {

            if msg.method() != req.method() {
                warn!("Got response with wrong method {} from {}@{} for {}",
                    msg.method(), from_id, from, req.method());

                locked.respond_wrong_method(msg);
                if let Some(detector) = self.suspicious_node_detector.as_ref() {
                    detector.lock().unwrap().malformed_message(from.clone());
                }
                return None;
            }

            // Remove all to prevent timeout race, defense against timeout race.
            let removed = self.pending_calls.remove(&msg.txid());
            if removed.is_none() {
                warn!("No pending request found for response {} with txid {}, maybe already timed out, discard the response",
                    msg.method(), msg.txid());
                return None;
            }

            msg.set_associated_call(call.clone());
            // locked.respond(&msg);
            // if !locked.is_reachable_at_creation() {
                // TODO:
            //}
            return Some(msg);
        }

        // Handle inconsistent socket (e.g., NAT issues or attack)
        // - the message is not a request
        // - the transaction ID matched
        // - response source did not match request destination
        // this happening by chance is exceedingly unlikely indicates either port-mangling NAT,
        // a multihomed host listening on any-local address or some kind of attack
        warn!("Node address not consistent, ignored. request: {} <- response: {}@{}",
                call.lock().unwrap().target().id(), from_id, from);

        // TODO: handle suspicious stuff.
        if let Some(detector) = self.suspicious_node_detector.as_ref() {
            detector.lock().unwrap().inconsistent(from.clone(), Some(from_id));
        }

        locked.respond_inconsistent_socket(msg);
        None
    }
}

impl fmt::Display for RpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
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

#[allow(dead_code)]
pub(crate) struct RunLoopContext {
    timer_queue : DelayQueue<timer::TimerId>,
    jobs        : HashMap<u64, Job>,
    keys        : HashMap<u64, Key>,

    rx_channel  : UnboundedReceiver<Command>,
    rx_socket   : Arc<UdpSocket>,
    tx_server   : Arc<Mutex<RpcServer>>,
    dht         : Arc<Mutex<DHT>>,
}

#[allow(dead_code)]
impl RunLoopContext {
    pub(crate) fn new(rpc_server: Arc<Mutex<RpcServer>>, dht: Arc<Mutex<DHT>>) -> Self {
        Self {
            timer_queue : DelayQueue::new(),
            jobs        : HashMap::new(),
            keys        : HashMap::new(),

            rx_channel  : rpc_server.lock().unwrap().rx_channel_take(),
            rx_socket   : rpc_server.lock().unwrap().rx_socket_take(),
            tx_server   : rpc_server.clone(),
            dht,
        }
    }

    fn rx_channel(&mut self) -> &mut UnboundedReceiver<Command> {
        &mut self.rx_channel
    }

    fn rx_socket(&mut self) -> &Arc<UdpSocket> {
        &self.rx_socket
    }

    fn add_timer(&mut self, delay: Duration, job: Job) {
        let jobid = job.id;
        if let Some(key) = self.keys.remove(&jobid) {
            let _ = self.timer_queue.remove(&key);
        }

        let key = self.timer_queue.insert(jobid, delay);
        self.keys.insert(jobid, key);
        self.jobs.insert(jobid, job);
    }

    fn remove_timer(&mut self, job_id: u64, reply: oneshot::Sender<bool>) {
        let mut removed = false;
        if let Some(job) = self.jobs.remove(&job_id) {
            job.cancel();
            removed = true;
        }
        if let Some(key) = self.keys.remove(&job_id) {
            let _ = self.timer_queue.remove(&key);
            removed = true;
        }
        let _ = reply.send(removed);
    }

    fn stop_timers(&mut self, reply: oneshot::Sender<()>) {
        self.timer_queue.clear();
        self.jobs.clear();
        self.keys.clear();
        let _ = reply.send(());
    }

    fn has_timers(&self) -> bool {
        !self.jobs.is_empty() || !self.keys.is_empty()
    }

    pub(crate) async fn run_loop(mut self) {
        let mut buf = vec![0u8; 2048];
        let rpc_server = self.tx_server.clone();
        let dht = self.dht.clone();

        loop {
            tokio::select! {
                biased;

                command = self.rx_channel.recv() => {
                    match command {
                        Some(Command::Add { delay, job }) => {
                            self.add_timer(delay, job);
                        }
                        Some(Command::Remove { job_id, reply }) => {
                            self.remove_timer(job_id, reply);
                        }
                        Some(Command::Stop { reply }) => {
                            self.stop_timers(reply);
                            break;
                        }
                        None => {
                            if !self.has_timers() {
                                break;
                            }
                        }
                    }
                }
                packet = self.rx_socket.recv_from(&mut buf) => {
                    let (len, from) = match packet {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Rpc server failed to receive packet:{e}");
                            continue;
                        }
                    };

                    let msg = rpc_server.lock().unwrap()
                        .parse_packet(&buf[..len], &from);

                    if let Some(msg) = msg {
                        dht.lock().unwrap().on_message(&msg);
                    }
                }
                else => break,
            }
        }
    }
}
