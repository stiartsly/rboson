use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, Duration};
use std::collections::{HashSet, HashMap};

use log::{info, warn, error, trace};
use tokio::{
    task,
    net::UdpSocket,
};

use crate::{
    Id,
    Network,
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
    rpccall::RpcCall,
    msg::msg::Message,
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

    message_cb      : Option<Box<dyn Fn(Arc<Mutex<Message>>) + Send>>,
    call_sent_cb    : Option<Box<dyn Fn(Arc<Mutex<RpcCall>>) + Send>>,
    call_timeout_cb : Option<Box<dyn Fn(Arc<Mutex<RpcCall>>) + Send>>,

    start_time      : Option<SystemTime>,
    is_running      : bool,

    worker          : Option<task::JoinHandle<()>>,
    socket          : Option<UdpSocket>,
}

impl RpcServer {
    const MAX_ACTIVE_CALLS: usize = 64;

    pub(crate) const RPC_CALL_TIMEOUT_MAX: u64 = 10 * 1000;

    pub(crate) fn new(
        sock_addr: &SocketAddr,
        identity: Arc<Mutex<CryptoIdentity>>,
        suspicious_node_detector: Option<DefaultSuspiciousNodeDetector>,
    ) -> Self {

        Self {
            identity,
            socket_addr: sock_addr.clone(),
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

            worker: None,
            socket: None,
        }
    }

    fn identity(&self) -> Arc<Mutex<CryptoIdentity>> {
        self.identity.clone()
    }

    pub(crate) fn has_pending_calls(&self) -> bool {
        self.pending_calls.len() > 0
    }

    pub(crate) fn id(&self) -> Id {
        self.identity.lock().unwrap().id().clone()
    }

    pub(crate) fn network(&self) -> Network {
        Network::from(self.socket_addr())
    }

    pub(crate) fn socket_addr(&self) -> &SocketAddr {
        &self.socket_addr
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

    pub(crate) fn is_running(&self) -> bool {
        self.is_running
    }

    pub(crate) fn age(&self) -> Duration {
        unimplemented!()
    }

    pub(crate) fn set_message_cb<F>(&mut self, cb: F)
    where
        F: Fn(Arc<Mutex<Message>>) + Send + 'static,
    {
        self.message_cb = Some(Box::new(cb));
    }

    pub(crate) fn set_call_sent_cb<F>(&mut self, cb: F)
    where
        F: Fn(Arc<Mutex<RpcCall>>) + Send + 'static,
    {
        self.call_sent_cb = Some(Box::new(cb));
    }

    pub(crate) fn set_call_timeout_cb<F>(&mut self, cb: F)
    where
        F: Fn(Arc<Mutex<RpcCall>>) + Send + 'static,
    {
        self.call_timeout_cb = Some(Box::new(cb));
    }

    pub(crate) async fn start(server: Arc<Mutex<Self>>) -> Result<()> {
        let socket = UdpSocket::bind(
            server.lock().unwrap().socket_addr()
        ).await.map_err(|e| {
            NetworkError::new(format!("{e}"))
        })?;

        _ = Some(task::spawn_blocking(async move || {
            loop {

                server.lock().unwrap().is_running = true;
                server.lock().unwrap().reachable = true;
                server.lock().unwrap().start_time = Some(SystemTime::now());

                info!("RPC server started at {}",
                    server.lock().unwrap().socket_addr()
                );

                let mut buff = vec![0u8; 1024];
                let rc = socket.recv_from(&mut buff).await;
                let (len, from) = match rc {
                    Ok(v) => v,
                    Err(e) => {
                        error!("DHT RPC server datagram socket error: {e}");
                        continue;
                    }
                };

                let cloned = server.clone();
                Self::handle_packet(cloned, &buff[..len], &from).await;
            }
        }));

        Ok(())
    }

    pub(crate) fn stop(server: Arc<Mutex<Self>>) {
        let worker = server.lock().unwrap().worker.take();
        if worker.is_none() {
            return;
        }
        if !server.lock().unwrap().is_running {
            return;
        }

        server.lock().unwrap().is_running = false;
        server.lock().unwrap().reachable = false;
        server.lock().unwrap().start_time = None;

        server.lock().unwrap().pending_calls.clear();

        info!("RPC server stopped at {}", server.lock().unwrap().socket_addr());
    }

    async fn handle_packet(server: Arc<Mutex<RpcServer>>, data: &[u8], from: &SocketAddr) {

        if data.len() < Id::BYTES + CryptoBox::MAC_BYTES + Message::MIN_BYTES {
            warn!("Ignored invalid packet(too short) from {}", from);

            // TODO: handle suspicious node
            return;
        }

        // Extract and validate remote ID
        let fromid = Id::try_from(&data[0.. Id::BYTES]).unwrap();
        let identity = server.lock().unwrap().identity();

        let rc = identity.lock().unwrap().decrypt_into(&fromid, &data[Id::BYTES ..]);
        let plain = match rc {
            Ok(v) => v,
            Err(err) => {
                warn!("Decrypt packet from {} error: {}, ignored it", from, err);
                return;
            }
        };

        let mut msg = match serde_cbor::from_slice::<Message>(&plain) {
            Ok(msg) => msg,
            Err(err) => {
                warn!("Got a wrong packet from {} with {}, ignored it", from, err);
                return;
            }
        };

        msg.set_id(fromid);
        msg.set_remote(fromid.clone(), from.clone());

        trace!("Received {}:{} from {}@{}: {}", msg.method(), msg.kind(),
				fromid, from, msg);


        // Handle request message
        if msg.is_req() {
            // TODO: handle metric stuff.

            let locked_server = server.lock().unwrap();
            let cb = locked_server.message_cb.as_ref();
            if let Some(cb) = cb {
                cb(Arc::new(Mutex::new(msg)));
            }
            drop(locked_server);
            return;
        }

        let locked_server = server.lock().unwrap();
        let call = locked_server.pending_calls.get(&msg.txid()).cloned();
        drop(locked_server);

        if let Some(call) = call {
            let req = call.lock().unwrap().req();
            if req.lock().unwrap().remote_addr() == from {

                if msg.method() != req.lock().unwrap().method() {

                    warn!("Got response with wrong method {} from {}@{} for {}",
                        msg.method(), fromid, from, req.lock().unwrap().method());

                    // call.lock().unwrap().respond_wrong_method();
                    // TODO: suspicious node handling

                    return;
                }

                // Remove all to prevent timeout race, defense against timeout race.
                let removed = server.lock().unwrap().pending_calls.remove(&msg.txid()).is_some();
                if !removed {
                    call.lock().unwrap().respond(
                        Arc::new(Mutex::new(msg))
                    );

                    let locked_server = server.lock().unwrap();
                    let cb = locked_server.call_sent_cb.as_ref();
                    if let Some(cb) = cb {
                        cb(call.clone());
                    }
                    drop(locked_server);

                    // TODO: handle metric stuff.
                }

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

            // TODO: handle metric stuff.

			// but expect an upcoming timeout if it's really just a misbehaving node
            call.lock().unwrap().respond_inconsistent_socket(
                Arc::new(Mutex::new(msg))
            );
			return;
        }

        // TODO: handle suspicious stuff.
        // TODO: handle metric stuff.
    }

    pub(crate) async fn send_call(&mut self, call: Arc<Mutex<RpcCall>>) {
        if self.pending_calls.len() >= Self::MAX_ACTIVE_CALLS {
            error!("Too many active calls, cannot send the call.");
            return;
        }

        let msg  = call.lock().unwrap().req();
        let txid = call.lock().unwrap().txid();

        self.pending_calls.insert(txid, call.clone());

        match self.send_msg(msg).await {
            Ok(_) => {
                call.lock().unwrap().sent();
                if let Some(cb) = self.call_sent_cb.as_ref() {
                    cb(call);
                }
            },
            Err(e) => {
                self.pending_calls.remove(&txid);
                call.lock().unwrap().fail(e);
            }
        }

        unimplemented!()
    }

    pub(crate) async fn send_msg(&mut self, msg: Arc<Mutex<Message>>) -> Result<()> {
        msg.lock().unwrap().set_id(self.id());

        let locked_msg = msg.lock().unwrap();
        let plain = serde_cbor::to_vec(&*locked_msg).map_err(|e| {
            ProtocolError::new(format!("INTERNAL ERROR: failed to serialize message: {e}"))
        })?;

        let encrypted = self.identity().lock().unwrap().encrypt_into(
            locked_msg.remote_id(),
            &plain
        ).map_err(|e| {
            CryptoError::new(format!("INTERNAL ERROR: failed to encrypt message: {e}"))
        })?;

        let mut buf = Vec::new() as Vec<u8>;
        buf.extend_from_slice(msg.lock().unwrap().id().as_bytes());
        buf.extend_from_slice(&encrypted);

        let socket = match self.socket.as_ref() {
            Some(s) => s,
            None => return Err(NetworkError::new("INTERNAL ERROR: socket not initialized".into())),
        };

        if let Err(e) = socket.send_to(&buf, msg.lock().unwrap().remote_addr()).await {
            return Err(NetworkError::new(format!("INTERNAL ERROR: failed to send message: {}", e)));
        };

        Ok(())
    }
}
