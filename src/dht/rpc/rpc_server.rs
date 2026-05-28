use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{SystemTime, Duration},
    collections::{HashSet, HashMap},
    future::Future,
    pin::Pin,
};

use futures::StreamExt;
use tokio::{
    sync::{mpsc, oneshot},
   //task,
    task::JoinHandle,
};
use tokio_util::time::{delay_queue::Key, DelayQueue};

use log::{info, warn, error, trace};
use tokio::{
    task,
    net::UdpSocket,
    sync::mpsc::{UnboundedSender, UnboundedReceiver},
};

use crate::{
    Id,
    CryptoBox,
    CryptoIdentity,
    errors::{
        Result,
        NetworkError,
        ProtocolError,
        CryptoError
    }
};

use crate::dht::{
    msg::msg::Message,
    timer::{self, Job, Command},
    rpc::rpccall::RpcCall,
    suspicious_node_detector::{
        SuspiciousNodeDetector,
        DefaultSuspiciousNodeDetector
    },
};

pub(crate) struct RpcServer {
    identity        : Arc<Mutex<CryptoIdentity>>,
    socket_addr     : SocketAddr,

    suspicious_node_detector: Option<DefaultSuspiciousNodeDetector>,

    pending_calls   : HashMap<i32, Arc<Mutex<RpcCall>>>,

    recv_pkts       : u32,
    recv_pkts_at_last_reachable_check: u32,

    last_reachable_check: Option<SystemTime>,
    reachable       : bool,

    reachable_cb    : Option<Box<dyn Fn(bool) + Send>>,
    message_cb      : Option<Box<dyn Fn(&mut Message) -> () + Send>>,
    call_sent_cb    : Option<Box<dyn Fn(Arc<Mutex<RpcCall>>) -> () + Send>>,
    call_timeout_cb : Option<Box<dyn Fn(Arc<Mutex<RpcCall>>) -> () + Send>>,

    start_time      : Option<SystemTime>,
    is_running      : bool,

    client          : timer::Client,
    tx_channel      : UnboundedSender<Command>,
    rx_channel      : Option<UnboundedReceiver<Command>>,

    tx_socket       : Option<UdpSocket>,
    rx_socket       : Option<UdpSocket>,
}

impl RpcServer {
    const MAX_ACTIVE_CALLS: usize = 64;

    pub(crate) const RPC_CALL_TIMEOUT_MAX: u64 = 10 * 1000;

    pub(crate) fn new(
        socket_addr: SocketAddr,
        identity: Arc<Mutex<CryptoIdentity>>,
        suspicious_node_detector: Option<DefaultSuspiciousNodeDetector>,
    ) -> Self {

        let (tx, rx) = mpsc::unbounded_channel::<Command>();
        Self {
            identity,
            socket_addr,
            suspicious_node_detector,
            pending_calls: HashMap::new(),

            recv_pkts: 0,
            recv_pkts_at_last_reachable_check: 0,
            last_reachable_check: None,
            reachable: false,

            reachable_cb    : None,
            message_cb      : None,
            call_sent_cb    : None,
            call_timeout_cb : None,

            start_time: None,
            is_running: false,

            client          : timer::Client::new(),

            tx_channel: tx,
            rx_channel: Some(rx),

            tx_socket: None,
            rx_socket: None,
        }
    }

    fn identity(&self) -> Arc<Mutex<CryptoIdentity>> {
        self.identity.clone()
    }

    fn take_rx_channel(&mut self) -> UnboundedReceiver<Command> {
        self.rx_channel.take().expect("unbound channel should be created.")
    }

    fn take_rx_socket(&mut self) -> UdpSocket {
        self.rx_socket.take().expect("UDP socket should be created.")
    }

    pub(crate) fn has_pending_calls(&self) -> bool {
        self.pending_calls.len() > 0
    }

    pub(crate) fn set_reachable(&mut self, reachable: bool) {
        if self.reachable == reachable {
            return;
        }

        self.reachable = reachable;
        if let Some(cb) = self.reachable_cb.as_ref() {
            cb(reachable);
        }
    }

    pub(crate) fn set_reachable_cb<F>(&mut self, cb: F)
    where
        F: Fn(bool) + Send + 'static,
    {
        self.reachable_cb = Some(Box::new(cb));
    }

    pub(crate) fn is_reachable(&self) -> bool {
        self.reachable
    }

    pub(crate) fn age(&self) -> Duration {
        unimplemented!()
    }

    pub(crate) fn set_message_cb<F>(&mut self, cb: F)
    where F: Fn(&mut Message) -> () + Send + 'static,
    {
        self.message_cb = Some(Box::new(cb));
    }

    pub(crate) fn set_call_sent_cb<F>(&mut self, cb: F)
    where F: Fn(Arc<Mutex<RpcCall>>) -> () + Send + 'static,
    {
        self.call_sent_cb = Some(Box::new(cb));
    }

    pub(crate) fn set_call_timeout_cb<F>(&mut self, cb: F)
    where F: Fn(Arc<Mutex<RpcCall>>) -> () + Send + 'static,
    {
        self.call_timeout_cb = Some(Box::new(cb));
    }

    pub(crate) fn start(&mut self) -> Result<()> {
        let socket_addr = self.socket_addr.clone();
        let std_socket = match std::net::UdpSocket::bind(socket_addr) {
            Ok(socket) => socket,
            Err(e) => {
                error!("Rpc server failed to bind udp socket at {}: {e}", socket_addr);
                return Err(NetworkError::new(format!("{e}")));
            }
        };

        if let Err(e) = std_socket.set_nonblocking(true) {
            error!("Rpc server failed to configure udp socket at {}: {e}", socket_addr);
            return Err(NetworkError::new(format!("{e}")));
        }

        let send_std = match std_socket.try_clone() {
            Ok(socket) => socket,
            Err(e) => {
                error!("Rpc server failed to clone udp socket at {}: {e}", socket_addr);
                return Err(NetworkError::new(format!("{e}")));
            }
        };

        let rx_socket = match UdpSocket::from_std(std_socket) {
            Ok(socket) => socket,
            Err(e) => {
                error!("Rpc server failed to create async rx socket for {}: {e}", socket_addr);
                return Err(NetworkError::new(format!("{e}")));
            }
        };

        let tx_socket = match UdpSocket::from_std(send_std) {
            Ok(socket) => socket,
            Err(e) => {
                error!("Rpc server failed to create async tx socket for {}: {e}", socket_addr);
                return Err(NetworkError::new(format!("{e}")));
            }
        };

        {
            self.rx_socket = Some(rx_socket);
            self.tx_socket = Some(tx_socket);
        }

        self.is_running = true;
        Ok(())
    }

    pub(crate) fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        // Signal the run_loop to stop (fire-and-forget; don't wait for reply)
        let (reply_tx, _) = oneshot::channel();
        let _ = self.tx_channel.send(Command::Stop { reply: reply_tx });

        self.pending_calls.clear();

        self.tx_socket = None;
        self.rx_socket = None;

        self.is_running = false;
        self.reachable = false;
        self.start_time = None;

        info!("RPC server stopped at {}", &self.socket_addr);
    }

    pub(crate) async fn send_call(&mut self, call: Arc<Mutex<RpcCall>>) {
        if self.pending_calls.len() >= Self::MAX_ACTIVE_CALLS {
            error!("Too many active calls pending in the queue.");
            return;
        }

        let txid = call.lock().unwrap().txid();
        self.pending_calls.insert(txid, call.clone());

        let mut locked = call.lock().unwrap();
        let mut msg = locked.req_mut();
        match self.send_msg(&mut msg).await {
            Ok(_) => {
                locked.sent();
                //TODO: if let Some(cb) = self.call_sent_cb.as_ref() {
                //    cb(call);
                //}
            },
            Err(e) => {
                self.pending_calls.remove(&txid);
                call.lock().unwrap().fail(e);
            }
        }
    }

    pub(crate) async fn send_msg(&mut self, msg: &mut Message) -> Result<usize> {
        let id = self.identity.lock().unwrap().id().clone();
        msg.set_id(id);

        let plain = serde_cbor::to_vec(&msg).map_err(|e| {
            ProtocolError::new(format!("Error: failed to serialize message: {e}").into())
        })?;

        let encrypted = self.identity().lock().unwrap().encrypt_into(
            msg.remote_id(),
            &plain
        ).map_err(|e| {
            CryptoError::new(format!("Error: failed to encrypt message: {e}").into())
        })?;

        let mut data = Vec::with_capacity(encrypted.len() + Id::BYTES);
        data.extend_from_slice(msg.id());
        data.extend_from_slice(&encrypted);

        let socket = self.tx_socket.as_ref().expect("socket should be initialized");
        let len = socket.send_to(
            &data, msg.remote_addr()
        ).await.map_err(|e| {
            NetworkError::new(format!("Error: failed to send message: {}", e))
        })?;

        Ok(len)
    }
}

pub(crate) async fn run_loop(server: Arc<Mutex<RpcServer>>) {
    let mut rx = server.lock().unwrap().take_rx_channel();
    let socket = server.lock().unwrap().take_rx_socket();

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

                tokio::spawn(async move {
                    if job.is_active() {
                        job.invoke().await;
                    }
                });
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

    let data = identity.lock().unwrap()
        .decrypt_into(&fromid, &data[Id::BYTES ..]);
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

    msg.set_id(fromid);
    msg.set_remote(fromid.clone(), from.clone());

    trace!("Received {}:{} from {}@{}: {}", msg.method(), msg.kind(),
            fromid, from, msg);


    // Handle request
    if msg.is_req() {
        let mut server = server.lock().unwrap();
        let cb = server.message_cb.as_mut();
        if let Some(cb) = cb {
            cb(&mut msg);
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
            let mut server = server.lock().unwrap();
            let cb = server.message_cb.as_mut();
            if let Some(cb) = cb {
                cb(&mut msg);
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
