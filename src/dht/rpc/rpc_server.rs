use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{SystemTime, Duration},
    collections::HashMap,
};
use futures::StreamExt;
use log::{info, warn, error, trace};
use tokio_util::time::{delay_queue::Key, DelayQueue};
use tokio::{
    net::UdpSocket,
    task::JoinHandle,
    sync::oneshot,
    sync::mpsc::{self, UnboundedSender, UnboundedReceiver},
};

use crate::{
    Id,
    NodeInfo,
    CryptoBox,
    cryptobox::Nonce,
    Identity,
    CryptoIdentity,
    errors::{
        Result,
        NetworkError,
        ProtocolError,
        CryptoError
    }
};
use crate::dht::{
    consumer::Consumer,
    rpc::RpcCall,
    msg::Message,
    timer::{self, Job, Command},
    suspicious_node_detector::DefaultSuspiciousNodeDetector,
};

pub(crate) struct RpcServer {
    identity            : Arc<CryptoIdentity>,
    ni                  : Arc<NodeInfo>,

    suspicious_node_detector: Option<DefaultSuspiciousNodeDetector>,
    pending_calls       : HashMap<i32, Arc<Mutex<RpcCall>>>,

    recv_packets        : u32,
    recv_packets_at_last_reachable_check: u32,
    last_reachable_check: SystemTime,
    reachable           : bool,

    reachable_handler   : Option<Consumer<bool>>,
    message_handler     : Option<Box<dyn Fn(&mut Message) + Send>>,
    callsent_handler    : Option<Box<dyn Fn(&mut RpcCall) + Send>>,
    calltimeout_handler : Option<Box<dyn Fn(&mut RpcCall) + Send>>,

    start_time          : Option<SystemTime>,
    is_running          : bool,
    reachable_check_task: Option<JoinHandle<()>>,

    client              : timer::Client,
    tx_channel          : Option<UnboundedSender<Command>>,
    rx_channel          : Option<UnboundedReceiver<Command>>,

    tx_socket           : Option<Arc<UdpSocket>>,
    rx_socket           : Option<Arc<UdpSocket>>,
}

impl RpcServer {
    const MAX_ACTIVE_CALLS: usize = 64;
    pub(crate) const RPC_CALL_TIMEOUT_MAX: u64 = 10 * 1000;
    const REACHABILITY_CHECK_INTERVAL: Duration = Duration::from_millis(5_000);
    const REACHABILITY_TIMEOUT: Duration = Duration::from_millis(60_000);

    pub(crate) fn new(
        ni: Arc<NodeInfo>,
        identity: Arc<CryptoIdentity>,
        suspicious_node_detector: Option<DefaultSuspiciousNodeDetector>,
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

            client              : timer::Client::new(),
            tx_channel          : None,
            rx_channel          : None,
            tx_socket           : None,
            rx_socket           : None,
        }
    }

    fn identity(&self) -> Arc<CryptoIdentity> {
        self.identity.clone()
    }

    fn tx_socket(&self) -> &Arc<UdpSocket> {
        self.tx_socket.as_ref().expect("socket should be initialized")
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
    where F: Fn(&mut Message) + Send + 'static,
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


    pub(crate) async fn start(&mut self) -> Result<()> {
        let (tx, rx) = mpsc::unbounded_channel::<Command>();
        self.rx_channel = Some(rx);
        self.tx_channel = Some(tx);

        let socket_addr = self.ni.socket_addr().clone();
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

    pub(crate) async fn send_call(&mut self, call: RpcCall) {
        if self.pending_calls.len() >= Self::MAX_ACTIVE_CALLS {
            error!("Too many active calls pending in the queue.");
            return;
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
                locked.fail(e);
            }
        }
    }

    pub(crate) async fn send_msg(&mut self, msg: &mut Message) -> Result<usize> {
        let nodeid = self.ni.id().clone();
        msg.set_nodeid(nodeid);

        let plain = serde_cbor::to_vec(&msg).map_err(|e| {
            ProtocolError::new(format!("Serializing message error: {e}").into())
        })?;

        let len = Id::BYTES + Nonce::BYTES + CryptoBox::MAC_BYTES + plain.len();
        let mut buf = Vec::with_capacity(len);
        let len = self.identity.encrypt(
            msg.remote_id(), &plain, &mut buf[Id::BYTES..]
        ).map_err(|e| {
            CryptoError::new(format!("Encrypting message error: {e}").into())
        })?;

        buf[..Id::BYTES].copy_from_slice(msg.nodeid().as_bytes());

        let sent_len = self.tx_socket().send_to(
            &buf[..len + Id::BYTES], msg.remote_addr()
        ).await.map_err(|e| {
            NetworkError::new(format!("Sending message error: {}", e))
        })?;

        if sent_len != len + Id::BYTES {
            return Err(NetworkError::new(format!("Error: sent length {} does not match expected {}", sent_len, len + Id::BYTES)));
        }
        Ok(sent_len)
    }
}

pub(crate) async fn run_loop(server: Arc<Mutex<RpcServer>>) {
    let mut rx = server.lock().unwrap().rx_channel.take()
        .expect("channel should be initialized");
    let socket = server.lock().unwrap().rx_socket.take().
        expect("socket should be initialized");

    let mut queue = DelayQueue::new();
    let mut jobs = HashMap::<u64, Job>::new();
    let mut keys = HashMap::<u64, Key>::new();
    let mut buff = vec![0u8; 2048];

    loop {
        tokio::select! {
            biased;

            command = rx.recv() => {
                match command {
                    Some(Command::Add { delay, job }) => {
                        let jobid = job.id;
                        if let Some(key) = keys.remove(&jobid) {
                            let _ = queue.remove(&key);
                        }

                        let key = queue.insert(jobid, delay);
                        keys.insert(jobid, key);
                        jobs.insert(jobid, job);
                    }
                    Some(Command::Remove { job_id, reply }) => {
                        let mut removed = false;
                        if let Some(job) = jobs.remove(&job_id) {
                            job.cancel();
                            removed = true;
                        }

                        if let Some(key) = keys.remove(&job_id) {
                            let _ = queue.remove(&key);
                            removed = true;
                        }
                        let _ = reply.send(removed);
                    }
                    Some(Command::Stop { reply }) => {
                        jobs.clear();
                        keys.clear();
                        queue.clear();
                        let _ = reply.send(());
                        break;
                    }
                    None => {
                        if queue.is_empty() {
                            break;
                        }
                    }
                }
            }
            maybe_expired = queue.next(), if !queue.is_empty() => {
                let Some(expired) = maybe_expired else {
                    continue;
                };

                let jobid = expired.into_inner();
                keys.remove(&jobid);

                let Some(job) = jobs.get(&jobid).cloned() else {
                    continue;
                };

                if !job.is_active() {
                    jobs.remove(&jobid);
                    continue;
                }

                if let Some(interval) = job.interval {
                    if job.is_active() {
                        let key = queue.insert(jobid, interval);
                        keys.insert(jobid, key);
                    }
                } else {
                    jobs.remove(&jobid);
                }

                let _ = tokio::spawn(async move {
                    if job.is_active() {
                        job.invoke();
                    }
                }).await;
            }
            recv = socket.recv_from(&mut buff) => {
                let (len, from) = match recv {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Rpc server receive packet from udp socket error: {e}");
                        continue;
                    }
                };
                handle_packet(server.clone(), &buff[..len], &from).await;
            }
            else => break,
        }
    }
}

async fn handle_packet(
    server: Arc<Mutex<RpcServer>>,
    data: &[u8],
    from: &SocketAddr
) {
    if data.len() < Id::BYTES + CryptoBox::MAC_BYTES + Message::MIN_BYTES {
        warn!("Ignored invalid packet(too short) from {}", from);

        // TODO: handle suspicious node
        return;
    }

    let fromid = Id::try_from(&data[0.. Id::BYTES]).unwrap();
    let identity = server.lock().unwrap().identity();

    let data = identity.decrypt_into(&fromid, &data[Id::BYTES ..]);
    let decrypted = match data {
        Ok(v) => v,
        Err(err) => {
            warn!("Decrypting packet from {} error: {}, ignored it", from, err);
            return;
        }
    };
    let mut msg = match serde_cbor::from_slice::<Message>(&decrypted) {
        Ok(msg) => msg,
        Err(err) => {
            warn!("Deserialize packet from {} with {}, ignored it", from, err);
            return;
        }
    };

    msg.set_nodeid(fromid);
    msg.set_remote(fromid.clone(), from.clone());

    trace!("Received {}:{} from {}@{}: {}", msg.method(), msg.kind(),
            fromid, from, msg);

        server.lock().unwrap().recv_packets += 1;

    // Handle request
    if msg.is_req() {
        let server = server.lock().unwrap();
        if let Some(handler) = server.message_handler.as_ref() {
            handler(&mut msg);
        }
        drop(server);
        return;
    }

    // Handle response
    let call = server.lock().unwrap().pending_calls.get(&msg.txid()).cloned();
    let Some(call) = call else {
        // TODO: handle suspicious stuff.
        return;
    };

    let locked = call.lock().unwrap();
    let req = locked.req();
    if req.remote_addr() == from {

        if msg.method() != req.method() {
            warn!("Got response with wrong method {} from {}@{} for {}",
                msg.method(), fromid, from, req.method());

            call.lock().unwrap().respond_wrong_method(msg);
            // TODO: suspicious node handling
            return;
        }

        // Remove all to prevent timeout race, defense against timeout race.
        let removed = server.lock().unwrap().pending_calls.remove(&msg.txid());
        if removed.is_none() {
            return;
        }

        {
            let server = server.lock().unwrap();
            if let Some(handler) = server.message_handler.as_ref() {
                handler(&mut msg);
            }
            drop(server);
        }

        call.lock().unwrap().respond(msg);
        return;
    }

    // Handle inconsistent socket (e.g., NAT issues or attack)
    // - the message is not a request
    // - the transaction ID matched
    // - response source did not match request destination
    // this happening by chance is exceedingly unlikely indicates either port-mangling NAT,
    // a multihomed host listening on any-local address or some kind of attack
    warn!("Node address not consistent, ignored. request: {} <- response: {}@{}",
            call.lock().unwrap().target().id(), fromid, from);

    // TODO: handle suspicious stuff.

    call.lock().unwrap().respond_inconsistent_socket(msg);
}
