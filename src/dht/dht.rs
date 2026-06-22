use std::{
    pin::Pin,
    collections::{HashMap, HashSet},
    net::SocketAddr,
    time::SystemTime,
    path::{PathBuf, Path},
    error::Error as StdError,
    future::Future,
    sync::{
        Arc, Weak, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::task::JoinHandle;
use futures::stream::{FuturesUnordered, FuturesOrdered, StreamExt};
use log::{debug, info, warn, error};

use crate::{
    Id, Network,
    NodeInfo, PeerInfo, Value,
    crypto_identity::CryptoIdentity,
    errors::Result
};
use crate::dht::{
    utils::{is_any_unicast, is_bogon},
    ConnectionStatus,
    ConnectionStatusListener,
    promise::{Promise, PromiseFuture},
    consumer::{Consumer, AsyncConsumer},
    token_manager::TokenManager,
    lookup_option::LookupOption,
    timer_client::TimerClient,
    storage::data_storage::DataStorage,
    suspicious_node_detector::SuspiciousNodeDetector,
    rpc::{
        Reachability,
        RpcCall, rpccall::State as CallState,
        rpc_server::{self, RpcServer},
    },
    msg::{
        Message,
        LookupRequest, LookupResponse,
        msg::{*, Kind, Method, Body},
    },
    routing::{
        routing_table::RoutingTable,
        KClosestNodes,
        KBucketEntry,
        KBucket,
        Prefix,
    },
    task::{
        task::{State, Task},
        task_manager::TaskManager,
        task_listener::TaskListener,
        LookupTask,
        NodeLookupTask,
        PeerLookupTask,
        ValueLookupTask,
        PeerAnnounceTask,
        ValueAnnounceTask,
        PingRefreshTask
    }
};

#[derive(Default)]
pub(crate) struct Builder<'a> {
    identity        : Option<Arc<CryptoIdentity>>,
    storage         : Option<Arc<Mutex<dyn DataStorage>>>,
    token_man       : Option<Arc<TokenManager>>,
    timer_client    : Option<Arc<TimerClient>>,
    data_dir        : Option<&'a Path>,
    bootstrap_nodes : Option<&'a[NodeInfo]>,
    listener        : Option<Arc<dyn ConnectionStatusListener>>,
}

impl<'a> Builder<'a> {
    pub(crate) fn with_identity(mut self, identity: Arc<CryptoIdentity>) -> Self {
        self.identity = Some(identity);
        self
    }

    pub(crate) fn with_bootstrap(mut self, bootstrap_nodes: &'a [NodeInfo]) -> Self {
        self.bootstrap_nodes = Some(bootstrap_nodes);
        self
    }

    pub(crate) fn with_timer_client(mut self, timer_client: Arc<TimerClient>) -> Self {
        self.timer_client = Some(timer_client);
        self
    }

    pub(crate) fn with_storage(mut self, storage: Arc<Mutex<dyn DataStorage>>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub(crate) fn with_tokenman(mut self, token_man: Arc<TokenManager>) -> Self {
        self.token_man = Some(token_man);
        self
    }

    pub(crate) fn with_datadir(mut self, datadir: &'a Path) -> Self {
        self.data_dir = Some(datadir);
        self
    }

    pub(crate) fn with_listener(mut self, listener: Arc<dyn ConnectionStatusListener>)  -> Self {
        self.listener = Some(listener);
        self
    }

    pub(crate) fn build(&self, network: Network, host: &str, port: u16) -> Result<DHT> {
        let persist_file = self.data_dir.as_ref().map(|v| {
            let filename = match network {
                Network::IPv4 => "dht4.cache",
                Network::IPv6 => "dht6.cache",
            };
            v.join(filename)
        });
        DHT::new(self, network, host.into(), port, persist_file)
    }
}

pub(crate) struct DHT {
    identity            : Arc<CryptoIdentity>,
    network             : Network,
    host                : String,
    port                : u16,

    is_running          : bool,
    status              : ConnectionStatus,
    listener            : Arc<dyn ConnectionStatusListener>,

    storage             : Arc<Mutex<dyn DataStorage>>,
    tokenman            : Arc<TokenManager>,
    task_man            : Arc<TaskManager>,

    persist_file        : Option<PathBuf>,
    rt                  : Option<Arc<Mutex<RoutingTable>>>,

    bootstrap_nodes     : Vec<NodeInfo>,
    bootstrap_ids       : Vec<Id>,
    last_bootstrap      : SystemTime,
    bootstrapping       : AtomicBool,

    last_maintenance    : SystemTime,
    maintenance_tasks   : Arc<Mutex<HashSet<Prefix>>>,

    timer_client        : Arc<TimerClient>,

    rpc_server          : Option<Arc<Mutex<RpcServer>>>,
    rpc_task            : Option<JoinHandle<()>>,
    rpc_task_quit_flag  : Arc<Mutex<bool>>,

    suspicious_detector : Option<Arc<Mutex<dyn SuspiciousNodeDetector>>>,
    pub(crate) weak     : Weak<Mutex<DHT>>,
}

impl DHT {
    const BOOTSTRAP_MIN_INTERVAL: u64 = 4 * 60 * 1000;              // 4 minutes
    const SELF_LOOKUP_INTERVAL: u128 = 30 * 60 * 1000;              // 30 minutes
    const ROUTING_TABLE_PERSIST_INTERVAL: u64 = 10 * 60 * 1000;     // 10 minutes
    const ROUTING_TABLE_MAINTENANCE_INTERVAL: u128 = 4 * 60 * 1000; // 4 minutes
    const RANDOM_LOOKUP_INTERVAL: u64 = 10 * 60 * 1000;             // 10 minutes
    const RANDOM_PING_INTERVAL  : u64 = 10 * 1000;                  // 10 seconds

    const BOOTSTRAP_IF_LESS_THAN_X_ENTRIES: usize = 30;
    const USE_BOOTSTRAP_NODES_IF_LESS_THAN_X_ENTRIES: usize = 8;

    fn new(
        b: &Builder,
        network: Network, host: String, port: u16,
        persist_file: Option<PathBuf>
    ) -> Result<Self>
    {
        assert!(b.identity.is_some());
        assert!(b.storage.is_some());
        assert!(b.timer_client.is_some());
        assert!(b.token_man.is_some());
        assert!(b.data_dir.is_some());
        assert!(b.listener.is_some());

        let identity = b.identity.as_ref().unwrap().clone();
        let storage  = b.storage.as_ref().unwrap().clone();
        let tokenman = b.token_man.as_ref().unwrap().clone();
        let tclient  = b.timer_client.as_ref().unwrap().clone();
        let listener = b.listener.as_ref().unwrap().clone();

        let nodeid = identity.id().clone();
        let bootstrap_nodes = b.bootstrap_nodes.map(|nodes| nodes.to_vec())
            .unwrap_or_else(Vec::new);

        Ok( Self {
            identity,
            network,
            host,
            port,
            is_running          : false,
            status              : ConnectionStatus::Disconnected,
            listener,
            storage,
            tokenman,
            task_man            : Arc::new(TaskManager::new()),

            rt                  : None,
            persist_file,

            bootstrap_nodes,
            bootstrap_ids       : Vec::new(),
            last_bootstrap      : SystemTime::UNIX_EPOCH,
            last_maintenance    : SystemTime::UNIX_EPOCH,
            maintenance_tasks   : Arc::new(Mutex::new(HashSet::new())),
            bootstrapping       : AtomicBool::new(false),
            timer_client        : tclient,
            suspicious_detector : None,
            weak                : Weak::new(), // will be set later

            rpc_server          : None,
            rpc_task            : None,
            rpc_task_quit_flag  : Arc::new(Mutex::new(false)),
        })
    }

    pub(crate) fn network(&self) -> Network {
        self.network
    }

    pub(crate) fn ni(&self) -> NodeInfo {
        let ip = self.host.parse().unwrap();
        NodeInfo::new(
            self.identity.id().clone(),
            SocketAddr::new(ip, self.port),
        )
    }

    pub(crate) fn rpc_server(&self) -> &Arc<Mutex<RpcServer>> {
        self.rpc_server.as_ref().expect("RpcServer not initialized")
    }

    pub(crate) fn rt(&self) -> &Arc<Mutex<RoutingTable>> {
        self.rt.as_ref().expect("Routing table not initialized")
    }

    pub(crate) fn dht(&self) -> Arc<Mutex<Self>> {
        self.weak.upgrade().expect("DHT instance is dropped")
    }

    pub(crate) fn id(&self) -> &Id {
        self.identity.as_ref().id()
    }

    pub(crate) fn send_msg(&self, msg: Message) {
        let _ = self.rpc_server.as_ref().expect("Rpc server not initalized")
                    .lock().unwrap()
                    .send_msg(&msg)
                    .map_err(|e| {error!("{e}"); e})
                    .map(|_|());
    }

    pub(crate) fn send_call(&self, call: RpcCall) {
        let _ = self.rpc_server.as_ref().expect("Rpc server not initalized")
                    .lock().unwrap()
                    .send_call(call)
                    .map_err(|e| {error!("{e}"); e})
                    .map(|_|());
    }

    fn fill_home_bucket(&self, nodes: Vec<NodeInfo>, promise: Promise<()>){
        let mut task = Box::new(NodeLookupTask::new(
            self.dht(),
            self.id().clone(),
            false,
        ));
        task.with_name("Bootstrap: filling home bucket".into());
        task.with_bootstrap(true);
        task.with_inject_candidates(nodes);
        task.with_listener(
            TaskListener::default().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );
        self.task_man.add(task);
    }

    fn fill_buckets(&self) ->  FuturesUnordered<PromiseFuture<()>> {
        let (number_of_entries, buckets) = {
            let locked_rt  = self.rt().lock().unwrap();
            (
                locked_rt.number_of_entries(),
                locked_rt.buckets(),
            )
        };

        let futures = FuturesUnordered::new();
        for bucket in buckets {
            if bucket.lock().unwrap().is_full() &&
                number_of_entries >= Self::BOOTSTRAP_IF_LESS_THAN_X_ENTRIES {
                continue;
            }

            let (lookup_target, bucket_prefix) = {
                let mut locked = bucket.lock().unwrap();
                locked.update_refresh_time();

                let prefix = locked.prefix().clone();
                (prefix.random_id(), prefix)
            };

            let (promise,future) = Promise::<()>::pair();
            let mut task = Box::new(NodeLookupTask::new(
                self.dht(),
                lookup_target,
                false,
            ));
            task.with_name(format!("Bootstrap: filling Bucket - {}", bucket_prefix));
            task.with_listener(
                TaskListener::default().ended_fn(
                    move |_| promise.complete(Ok(()))
                )
            );
            self.task_man.add(task);
            futures.push(future);
        }
        futures
    }

    fn try_ping_maintenance(&self,
        bucket: Arc<Mutex<KBucket>>,
        check_all: bool,
        remove_on_timeout: bool,
        _probe_replacement: bool,
        name: String
    ) {
        if !self.rpc_server().lock().unwrap().is_reachable() {
            return;
        }

        let (prefix, need_refresh, need_replacement) = {
            let locked = bucket.lock().unwrap();
            (
                locked.prefix().clone(),
                locked.needs_refreshing(),
                locked.needs_replacement()
            )
        };

        if self.maintenance_tasks.lock().unwrap().contains(&prefix) {
            return;
        }

        if need_refresh || need_replacement  {
            let mut task = Box::new(PingRefreshTask::new(self.dht()));
            task.with_name(name);
            task.with_check_all(check_all);
            task.with_remove_on_timeout(remove_on_timeout);
            task.with_bucket(bucket);

            if self.maintenance_tasks.lock().unwrap().insert(prefix) {
                let maintenance_tasks = self.maintenance_tasks.clone();
                task.with_listener(
                    TaskListener::default().ended_fn(move |_| {
                        maintenance_tasks.lock().unwrap().remove(&prefix);
                }));
                self.task_man.add(task);
            }
        }
    }

    pub(crate) fn random_lookup(&mut self) {
        if !self.rpc_server().lock().unwrap().is_reachable() {
            debug!("Periodic: not performing random lookup, server is unreachable");
            return;
        }

        let mut task = Box::new(NodeLookupTask::new(
            self.dht(),
            Id::random(),
            false,
        ));
        task.with_name("Periodic: random node lookup".into());
        self.task_man.add(task);
    }

    pub(crate) fn random_ping(&mut self) {
        let has_pending_calls =
            self.rpc_server().lock().unwrap().has_pending_calls();

        if has_pending_calls {
            info!("Periodic: not performing random ping, server has pending calls.");
            return;
        }

        let Some(entry) = self.rt().lock().unwrap().random_entry() else {
            debug!("Periodic: not performing random ping, routing table is empty.");
            return;
        };

        debug!("Periodic: random ping ...");

        let call = RpcCall::new(entry, ping_request());
        let _ = self.send_call(call);
    }

    fn routing_table_maintenance(&mut self) {
        if crate::elapsed_ms!(self.last_maintenance) <
                Self::ROUTING_TABLE_MAINTENANCE_INTERVAL {
            return;
        }

        debug!("Routing table maintenance ...");
        self.last_maintenance = SystemTime::now();

        let dht = self.dht();
        let rt  = self.rt().clone();
        let _ = RoutingTable::maintenance(
            rt,
            self.bootstrap_ids.as_slice(),
            Consumer::new(move |bucket: &Arc<Mutex<KBucket>>| {
                let prefix = bucket.lock().unwrap().prefix().clone();
                dht.lock().unwrap()
                    .try_ping_maintenance(bucket.clone(), false, false, false,
                        format!("Routing table maintenance: refreshing bucket {}", prefix)
                    );
            })
        );
    }

    async fn update(&mut self) {
        if !self.is_running {
            return;
        }

        debug!("Periodic: DHT update...");
        self.routing_table_maintenance();

        // bootstraping process.
        let entry_sz = self.rt().lock().unwrap().number_of_entries();
        if entry_sz >= Self::BOOTSTRAP_IF_LESS_THAN_X_ENTRIES &&
            crate::elapsed_ms!(self.last_bootstrap) <= Self::SELF_LOOKUP_INTERVAL {
            return;
        }

        let mut bootstrap_nodes = Vec::new();
        if entry_sz < Self::USE_BOOTSTRAP_NODES_IF_LESS_THAN_X_ENTRIES {
            bootstrap_nodes = self.bootstrap_nodes.clone();
        }

        let dht = self.dht();
        DHT::do_bootstrap(dht, bootstrap_nodes).await;

    }

    fn set_status(&mut self, status: ConnectionStatus) {
        if self.status == status {
            return;
        }
        let old = self.status;
        self.status = status;

        info!("DHT {}:{} connection status changed: {} => {}",
            self.network,
            self.identity.id(),old, self.status
        );

        let l = &self.listener;
        l.status_changed(self.network, self.status, old);
        match status {
            ConnectionStatus::Connecting    => l.connecting(self.network),
            ConnectionStatus::Connected     => l.connected(self.network),
            ConnectionStatus::Disconnected  => l.disconnected(self.network)
        }
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        info!("Starting DHT/{}:{} on {}:{} ...", self.network, self.id(), self.host, self.port);

        let mut rt = RoutingTable::new(self.id().clone());
        let mut _need_ping_from_cached_rt = false;
        if let Some(ref path) = self.persist_file {
            let file = path.display();
            let suc_cb = |_| debug!("Loaded routing table from {}.", file);
            let err_cb = |e| warn! ("Loading routing table from {} error: {e}", file);

            debug!("Loading routing table from {}.", file);
            let result = rt.load(&path)
                .map(suc_cb)
                .map_err(err_cb);

            _need_ping_from_cached_rt = result.is_ok() && !rt.is_empty();
        };
        self.rt = Some(Arc::new(Mutex::new(rt)));

        // initialize RPC server
        let mut server = RpcServer::new(
            self.ni(),
            self.identity.clone(),
            self.suspicious_detector.clone()
        );

        let dht = self.dht();
        server.message_handler(AsyncConsumer::new(move |msg:Arc<Message>| {
            let dht = dht.clone();
            Box::pin(async move {
                dht.lock().unwrap().on_message(&msg);
            })
        }));

        let rt = self.rt().clone();
        server.callsent_handler(Consumer::new({
            let rt = rt.clone();
            move |call: &RpcCall|{
                let nodeid = call.target_id();
                rt.lock().unwrap().on_request_sent(&nodeid);
            }
        }));
        server.calltimeout_handler(Consumer::new({
            let rt = rt.clone();
            move |call: &RpcCall| {
                let nodeid = call.target_id();
                rt.lock().unwrap().on_timeout(&nodeid);
            }
        }));
        server.start().await?;
        let dht = self.dht();
        server.reachable_handler(AsyncConsumer::new(move |reachable: bool|{
            let dht = dht.clone();
            Box::pin(async move {
                let mut locked = dht.lock().unwrap();

                if reachable {
                    locked.set_status(ConnectionStatus::Connected);
                } else {
                    locked.random_ping();
                    locked.set_status(ConnectionStatus::Disconnected);
                }
            })
        }));

        self.rpc_server = Some(Arc::new(Mutex::new(server)));
        self.set_status(ConnectionStatus::Connecting);

        let server = self.rpc_server().clone();
        let quit   = self.rpc_task_quit_flag.clone();
        let taskm  = self.task_man.clone();
        let task   = tokio::task::spawn_blocking(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build().expect("no rpc server engine runtime build")
                .block_on(async move {
                    rpc_server::run_loop(server, taskm, quit).await;
                }
            )
        });

        self.setup_periodic_tasks().await?;
        self.rpc_task = Some(task);
        self.is_running = true;

        info!("Started DHT/{}:{} on {}:{}", self.network, self.id(), self.host, self.port);
        Ok(())
    }

    pub(crate) async fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Stopping DHT/{}:{} on {}:{}......",
            self.network, self.id(), self.host, self.port);

        self.is_running = false;
        self.bootstrapping.store(false, Ordering::SeqCst);
        self.set_status(ConnectionStatus::Disconnected);

        let rpc_server = self.rpc_server.take();
        let rpc_task   = self.rpc_task.take();

        let notified = self.task_man.notified().clone();
        if let Some(t) = rpc_task {
            debug!("Stopping RPC server task...");
            *self.rpc_task_quit_flag.lock().unwrap() = true;
            notified.notify_one();
            let _ = t.await;
            info!("RPC server task stopped.");
        }

        if let Some(s) = rpc_server {
            s.lock().unwrap().stop().await;
        }

        {
            debug!("Stopping task manager...");
            self.task_man.stop();
            info!("Task manager stopped.");
        }

        let path = self.persist_file.take();
        let rt   = self.rt.take();
        if let (Some(path), Some(rt)) = (path, rt) {
            let _ = rt.lock().unwrap().save(&path);
        }

        if let Some(detector) = self.suspicious_detector.take() {
            detector.lock().unwrap().purge();
        }

        info!("Stopped DHT {}:{} on {}:{}.",
            self.network, self.id(), self.host, self.port);
    }

    async fn setup_periodic_tasks(&self) -> Result<()> {
        let dht = self.dht();
        let _ = self.timer_client.add_timer(30*1000, Some(30*1000),
            AsyncConsumer::new(move |_| {
                let dht = dht.clone();
                Box::pin(async move {
                    dht.lock().unwrap().update().await;
                })
            })
        )?;

        let dht = self.dht();
        let _ = self.timer_client.add_timer(
            Self::RANDOM_LOOKUP_INTERVAL,
            Some(Self::RANDOM_LOOKUP_INTERVAL),
            AsyncConsumer::new(move |_| {
                let dht = dht.clone();
                Box::pin(async move {
                    dht.lock().unwrap().random_lookup();
                })
            })
        )?;

        let dht = self.dht();
        let _ = self.timer_client.add_timer(
            Self::RANDOM_PING_INTERVAL,
            Some(Self::RANDOM_PING_INTERVAL),
            AsyncConsumer::new(move |_| {
                let dht = dht.clone();
                Box::pin(async move {
                    dht.lock().unwrap().random_ping();
                })
            })
        )?;

        if let Some(detector) = self.suspicious_detector.as_ref() {
            let detector = detector.clone();
            let _ = self.timer_client.add_timer(60, Some(30),
                AsyncConsumer::new(move |_| {
                    let detector = detector.clone();
                    Box::pin(async move {
                        info!("Periodic: purging suspicious nodes ...");
                        detector.lock().unwrap().purge();
                    })
                }))?;
        }

        if let Some(path) = self.persist_file.clone() {
            let rt = self.rt().clone();
            let _  = self.timer_client.add_timer(
                120,
                Some(Self::ROUTING_TABLE_PERSIST_INTERVAL),
                AsyncConsumer::new(move |_| {
                    let rt = rt.clone();
                    let path = path.clone();
                    Box::pin(async move {
                        let _ = rt.lock().unwrap().save(&path);
                    })
                })
            )?;
        }
        Ok(())
    }

    fn received(&mut self, msg: &Message) {
        let inconsistent_suspicious = |addr: SocketAddr, id: Id| {
            warn!("Received a message from inconsistent node {}@{}, ignored the potential routing table update",
                id, addr);

            self.suspicious_detector.as_ref().map(|v|
                v.lock().unwrap().inconsistent(addr, Some(id))
            );
        };
        let last_known_id = |addr: SocketAddr| {
            self.suspicious_detector.as_ref().and_then(|v| {
                v.lock().unwrap().last_known_id(&addr).cloned()
            })
        };
        let observe_suspicious = |addr: SocketAddr, id: Id| {
            self.suspicious_detector.as_ref().map(|v|
                v.lock().unwrap().observe(addr, id)
            );
        };

        let allowed = match cfg!(feature = "devp") {
            true => is_any_unicast(&msg.remote_addr().ip()),
            false => !is_bogon(msg.remote_addr()),
        };
        if !allowed {
            info!("Received a message from spoofed address {}, ignored the potential
                  routing table operation", msg.remote_addr());
            return;
        }

        let (remote_id, remote_addr, remote_port) = (
            msg.remote_id().clone(),
            msg.remote_addr().clone(),
            msg.remote_addr().port()
        );

        let call_opt = msg.associated_call();
        if let Some(call) = call_opt.as_ref() {
            // we only want remote nodes with stable ports in our routing table,
            // so apply a stricter check here
            let call = call.lock().unwrap();
            if call.nodeid_mismatched() || call.addr_mismatched() {
                inconsistent_suspicious(remote_addr, remote_id);
                return;
            }
        }

        if let Some(ref known_id) = last_known_id(remote_addr) {
            if known_id != msg.nodeid() {
                // We already know a node with that address but with a different ID.
                // This might happen if one node changes its ID.
                // Force remove from the routing table to prevent suspicious behavior
                warn!("Received a message from suspicious node {}@{}, force-removing routing table entries because ID-change was detected; new ID {}",
                    remote_id, remote_addr, known_id);

                let mut locked_rt = self.rt().lock().unwrap();
                let removed = locked_rt.remove(&known_id).is_some();
                if  removed {
                    // Might be a pollution attack, check other entries in the same bucket too.
                    // In case the random pings can't keep up with scrubbing.
                    let bucket = locked_rt.bucket(&known_id);
                    let prefix = {
                        let locked = bucket.lock().unwrap();
                        let prefix = locked.prefix().clone();
                        let expected_prefix = Prefix::from(&known_id, prefix.depth());

                        // Checking the prefix is expected prefix given known ID.
                        if expected_prefix != prefix {
                            error!("The prefix {} of the known ID {} is expected to be {},
                                but the bucket prefix is {}, this might indicate a routing table corruption",
                                prefix, known_id, expected_prefix, prefix);
                        }
                        prefix
                    };

                    info!("Checking bucket {} after ID change was detected", prefix);

                    self.try_ping_maintenance(bucket.clone(), true, false, false,
                        format!("Checking bucket {} after ID change was detected", prefix));
                }

                let msgid = msg.nodeid();
                let removed = locked_rt.remove(msgid).is_some();
                if  removed {
                    // Might be a pollution attack, check other entries in the same bucket too.
                    // In case the random pings can't keep up with scrubbing.
                    let bucket = locked_rt.bucket(msgid);
                    let prefix = {
                        let locked = bucket.lock().unwrap();
                        let prefix = locked.prefix().clone();
                        let expected_prefix = Prefix::from(&known_id, prefix.depth());

                        // Checking the prefix is expected prefix given known ID.
                        if expected_prefix != prefix {
                            error!("The prefix {} of the known ID {} is expected to be {},
                                but the bucket prefix is {}, this might indicate a routing table corruption",
                                prefix, known_id, expected_prefix, prefix);
                        }
                        prefix
                    };

                    info!("Checking bucket {} after ID change was detected", prefix);
                    self.try_ping_maintenance(bucket.clone(), true, false, false,
                        format!("Checking bucket {} after ID change was detected", prefix));
                }

                inconsistent_suspicious(remote_addr, remote_id);
                return;
            }
        }

        let existing_opt = {
            let locked = self.rt().lock().unwrap();
            locked.bucket_entry(&remote_id)
        };

        if let Some(existing) = existing_opt.as_ref() {
            if  existing.socket_addr() != &remote_addr ||
                existing.socket_addr().port() != remote_port {
                inconsistent_suspicious(remote_addr, remote_id);
                return;
            }
        }

        observe_suspicious(remote_addr, remote_id);

        let mut new_entry = KBucketEntry::new(remote_id, remote_addr);
        new_entry.set_ver(msg.ver());

        if let Some(_call) = call_opt {
            let locked = _call.lock().unwrap();
            new_entry.on_responded(0); // TOOD: RTT.
            new_entry.update_last_sent(locked.sent_time().unwrap());
        }

        self.rt().lock().unwrap().put(new_entry.clone());

        // Optimize: not the standard Kademlia behavior
		// incoming request && the new entry is unreachable && the target bucket not full,
		// then try to do a ping request to the new entry check its availability.
        if existing_opt.is_none() && !new_entry.is_reachable(){
            // Verify the node, speed up the bootstrap process or make the bucket more reliable.
			// only if the new entry is unreachable and the bucket is not full yet
            let call = RpcCall::new(new_entry, ping_request());
            let _ = self.send_call(call);
        }
    }

    fn send_err(&mut self, method: Method, code: i32, str: &str) {
        let msg = error_msg(method, 0, code, str.into());
        // TODO: set remote id and addr
        self.send_msg(msg);
    }

    pub(crate) fn on_message(&mut self, msg: &Message) {
        if !self.is_running {
            return;
        }

        // ignore the messages from myself
        if self.id() == msg.nodeid() {
            return;
        }

        match msg.kind() {
            Kind::Error    => self.on_error(&msg),
            Kind::Request  => self.on_request(&msg),
            Kind::Response => self.on_response(&msg),
        };

        self.received(msg);
    }

    fn on_request(&mut self, msg: &Message) {
        debug!("Received a {} request message from {}/{}, txid {}",
            msg.method(),
            msg.remote_addr(),
            msg.remote_id(),
            msg.txid()
        );
        let method = msg.method();
        match method {
            Method::Ping        => self.on_ping(msg),
            Method::FindNode    => self.on_find_node(msg),
            Method::FindValue   => self.on_find_value(msg),
            Method::FindPeer    => self.on_find_peer(msg),
            Method::StoreValue  => self.on_store_value(msg),
            Method::AnnouncePeer=> self.on_announce_peer(msg),
            _                   => self.on_unknown_req(msg),
        }
    }

    fn on_response(&mut self, msg: &Message) {
        debug!("Received a {} response message from {}/{}, txid {}",
            msg.method(),
            msg.remote_addr(),
            msg.remote_id(),
            msg.txid()
        );
    }

    fn on_error(&mut self, msg: &Message) {
        let Some(Body::Error(err)) = msg.body() else {
            return;
        };

        warn!("Received an error message from {}/{} - {}:{}, txid {}",
            msg.remote_addr(),
            msg.remote_id(),
            err.code(),
            err.description(),
            msg.txid()
        );
    }

    fn on_unknown_req(&mut self, msg: &Message) {
        warn!("Received unknown request {} from {}/{}, txid {}, ignoring it",
            msg.method(),
            msg.remote_addr(),
            msg.remote_id(),
            msg.txid()
        );
    }

    fn on_ping(&mut self, req: &Message) {
        if req.body().is_some() {
            panic!("Panic: ping request should have no body");
        }

        let rsp = {
            let mut msg = ping_response(req.txid());
            msg.set_remote(*req.remote_id(), *req.remote_addr());
            msg.set_nodeid(*self.id());
            msg
        };
        self.send_msg(rsp);
    }

    fn fill_closest_nodes(&self, target: Id) -> Vec<NodeInfo> {
        let locked  = self.rt().lock().unwrap();
        let mut kns = KClosestNodes::new(
            &locked,
            target,
            KBucket::MAX_ENTRIES
        );
        kns.fill();
        kns.into()
    }

    fn on_find_node(&mut self, req: &Message) {
        let Some(Body::FindNodeRequest(body)) = req.body() else {
            return;
        };

        let network= self.network();
        let target = body.target().clone();
        let nodes4 = match body.want4() && network.is_ipv4() {
            true  => Some(self.fill_closest_nodes(target)),
            false => None
        };
        let nodes6 = match body.want6() && network.is_ipv6() {
            true  => Some(self.fill_closest_nodes(target)),
            false => None
        };
        let token  = match body.want_token() {
            true  => self.tokenman.generate_token(
                        req.nodeid(), req.remote_addr(), &target),
            false => 0
        };

        let rsp = {
            let txid = req.txid();
            let mut msg = find_node_response(txid, nodes4, nodes6, token);
            msg.set_remote(*req.remote_id(), *req.remote_addr());
            msg.set_nodeid(*self.id());
            msg
        };
        self.send_msg(rsp);
    }

    fn on_find_value(&mut self, req: &Message) {
        let Some(Body::FindValueRequest(body)) = req.body() else {
            return;
        };

        let result = self.storage.lock().unwrap().get_value(body.target());
        let existing = match result {
            Ok(v) => v,
            Err(e) => {
                warn!("Retrieve value for {} error: {}", body.target(), e);
                return;
            }
        };

        let mut value = None;
        if let Some(v) = existing {
            if v.is_mutable() || body.expected_seq() < 0 ||
                v.sequence_number() >= body.expected_seq() {
                value = Some(v);
            }
        }

        let txid = req.txid();
        let mut rsp = if let Some(value) = value {
            find_value_response(txid, value)
        } else {
            let network = self.network();
            let target = body.target().clone();
            let nodes4 = match body.want4() && network.is_ipv4() {
                true  => Some(self.fill_closest_nodes(target)),
                false => None
            };
            let nodes6 = match body.want6() && network.is_ipv6() {
                true  => Some(self.fill_closest_nodes(target)),
                false => None
            };
            find_value_response_with_nodes(txid, nodes4, nodes6)
        };

        rsp.set_remote(*req.remote_id(), *req.remote_addr());
        rsp.set_nodeid(*self.id());

        self.send_msg(rsp);
    }

    fn on_store_value(&mut self, req: &Message) {
        let Some(Body::StoreValueRequest(body)) = req.body() else {
            return;
        };

        let value = body.value();
        let value_id = value.id();
        let remote_addr = req.remote_addr().clone();

        let is_valid = self.tokenman.verify_token(
            body.token(), req.nodeid(), &remote_addr, &value_id
        );

        if !is_valid {
            warn!("Invalid token for store value request from {}", remote_addr);
            return;
        }
        if !value.is_valid() {
            warn!("Invalid value for store value request from {}", remote_addr);
            return;
        }

        let result = self.storage.lock().unwrap().get_value(&value_id);
        let local_value = match result {
            Ok(v) => v,
            Err(e) => {
                warn!("Retrieve existing value {} error: {}", value_id, e);
                return;
            }
        };

        if let Some(existing) = local_value {
            if existing.is_mutable() != value.is_mutable() {
                warn!("Rejecting value {}: cannot replace mismatched mutable/immutable", value_id);
                self.send_err(Method::StoreValue, 300,
                    "Cannot replace mismatched mutable/immutable value");
                return;
            }
            if value.sequence_number() < existing.sequence_number() {
                warn!("Rejecting value {}: sequence number {} is less than existing {}", value_id, value.sequence_number(), existing.sequence_number());
                self.send_err(Method::StoreValue, 300,
                    "Sequence number is less than existing value");
                return;
            }
            if body.expected_seq() >= 0 && existing.sequence_number() > body.expected_seq() {
                warn!("Rejecting value {}: existing sequence number {} is greater than expected {}", value_id, existing.sequence_number(), body.expected_seq());
                self.send_err(Method::StoreValue, 300,
                    "Existing sequence number is greater than expected");
                return;
            }
            if existing.has_private_key() && !value.has_private_key() {
                // Skip update if the existing value is owned by this node and the new value is not.
				// Should not throw NotOwnerException, just silently ignore to avoid disrupting valid operations.
                warn!("Rejecting value {}: cannot replace existing value owned by this node.", value_id);
                return;
            }
        }

        _ = self.storage.lock().unwrap().put_value(value.clone(), false);

        let rsp = {
            let mut msg = store_value_response(req.txid());
            msg.set_remote(*req.remote_id(), *req.remote_addr());
            msg.set_nodeid(*self.id());
            msg
        };

        self.send_msg(rsp);
    }

    fn on_find_peer(&mut self, req: &Message) {
        let Some(Body::FindPeerRequest(body)) = req.body() else {
            return;
        };

        let result = self.storage.lock().unwrap().get_peers_with_expected_seq(
            body.target(), body.expected_seq(), body.expected_count()
        );
        let peers = match result {
            Ok(v) => v,
            Err(e) => {
                warn!("Retrieve peers for {} error: {}", body.target(), e);
                return;
            }
        };

        let txid = req.txid();
        let mut rsp = if peers.is_empty() {
            let network= self.network();
            let target = body.target().clone();
            let nodes4 = match body.want4() && network.is_ipv4() {
                true  => Some(self.fill_closest_nodes(target)),
                false => None
            };
            let nodes6 = match body.want6() && network.is_ipv6() {
                true  => Some(self.fill_closest_nodes(target)),
                false => None
            };
            find_peer_response_with_nodes(txid, nodes4, nodes6)
        } else {
            find_peer_response(txid, peers)
        };

        rsp.set_remote(*req.remote_id(), *req.remote_addr());
        rsp.set_nodeid(*self.id());

        self.send_msg(rsp);
    }

    fn on_announce_peer(&mut self, req: &Message) {
        let Some(Body::AnnouncePeerRequest(body)) = req.body() else {
            return;
        };

        let peer = body.peer();
        let remote_addr = req.remote_addr().clone();
        let is_valid = self.tokenman.verify_token(
            body.token(), req.nodeid(), &remote_addr, peer.id()
        );

        if !is_valid {
            warn!("Invalid token for announce peer request from {}", remote_addr);
            return;
        }
        if !peer.is_valid() {
            warn!("Invalid peer for announce peer request from {}", remote_addr);
            return;
        }

        let result = self.storage.lock().unwrap().get_peer(
            peer.id(), peer.fingerprint()
        );
        let local_peers = match result {
            Ok(v) => v,
            Err(e) => {
                warn!("Retrieve existing peer {} error: {}", peer.id(), e);
                return;
            }
        };

        if let Some(existing) = local_peers {
            if peer.sequence_number() < existing.sequence_number() {
                warn!("Rejecting peer {}: sequence number {} is less than existing {}", peer.id(), peer.sequence_number(), existing.sequence_number());
                self.send_err(Method::AnnouncePeer, 300,
                    "Sequence number is less than existing value");
                return;
            }

            if body.expected_seq() >= 0 && existing.sequence_number() > body.expected_seq() {
                warn!("Rejecting peer {}: existing sequence number {} is greater than expected {}", peer.id(), existing.sequence_number(), body.expected_seq());
                self.send_err(Method::AnnouncePeer, 300,
                    "Existing sequence number is greater than expected");
                return;
            }

            if existing.has_private_key() && !peer.has_private_key() {
                // Skip update if the existing peer is owned by this node and the new peer is not.
				// Should not throw NotOwnerException, just silently ignore to avoid disrupting valid operations.
                warn!("Rejecting peer {}: cannot replace existing peer owned by this node.", peer.id());
                return;
            }
        }
        _ = self.storage.lock().unwrap().put_peer(peer.clone(), false);

        let rsp = {
            let mut msg = announce_peer_response(req.txid());
            msg.set_remote(*req.remote_id(), *req.remote_addr());
            msg.set_nodeid(*self.id());
            msg
        };

        self.send_msg(rsp);
    }

    pub(crate) async fn bootstrap(
        dht: Arc<Mutex<Self>>,
        nodes: Vec<NodeInfo>
    ) {
        let mut locked = dht.lock().unwrap();
        if !locked.is_running {
            warn!("Bootstrapping skipped: the DHT/{} instance is not running.", locked.network);
            return;
        }
        if nodes.is_empty() {
            warn!("Bootstrapping skipped: no bootstrapping nodes provided.");
            return;
        }

        locked.add_bootstrap_nodes(&nodes);
        if locked.bootstrapping.load(Ordering::Relaxed) {
            warn!("The DHT/{} instance is already bootstrapping.", locked.network);
            return;
        }

        locked.last_bootstrap = SystemTime::UNIX_EPOCH;
        // Todo: handle status.

        drop(locked);
        DHT::do_bootstrap(dht, nodes).await;
    }

    async fn do_bootstrap(dht: Arc<Mutex<Self>>, nodes: Vec<NodeInfo>) {
        let (self_id, network, server) = {
            let locked_dht = dht.lock().unwrap();

            if crate::elapsed_ms!(locked_dht.last_bootstrap) < Self::BOOTSTRAP_MIN_INTERVAL as u128 {
                return;
            }

            let locked_rt = locked_dht.rt().lock().unwrap();
            if nodes.is_empty() && locked_rt.is_empty() {
                warn!("no bootstrap nodes provided and routing table is empty.");
                return;
            }
            drop(locked_rt);

            if locked_dht.bootstrapping.swap(true, Ordering::Relaxed) {
                warn!("The DHT/{} instance is already bootstrapping.", locked_dht.network);
                return;
            }

            let network = locked_dht.network();
            let server  = locked_dht.rpc_server().clone();
            let self_id = locked_dht.id().clone();

            (self_id, network, server)
        };

        info!("DHT/{}:{} bootstrapping ...", network, self_id);

        let mut unordered = FuturesUnordered::new();
        for item in nodes {
            if item.id() == &self_id {
                continue;
            }
            let msg = find_node_request(
                Id::random(),
                network.is_ipv4(),
                network.is_ipv6(),
                Some(true)
            );

            let mut call = RpcCall::new(item, msg);
            let (promise, future) = Promise::<Vec<NodeInfo>>::pair();

            call.set_simple_listener(move |_call, _, cur| {
                if cur.is_final() {
                    let mut nodes = None;

                    if cur == CallState::Responded {
                        let Some(rsp) = _call.rsp() else {
                            promise.complete(Ok(vec![]));
                            return;
                        };
                        let Some(body) = rsp.body() else {
                            promise.complete(Ok(vec![]));
                            return;
                        };
                        let Body::FindNodeResponse(body) = body else {
                            promise.complete(Ok(vec![]));
                            return;
                        };
                        nodes = body.nodes(network).map(|v| v.to_vec());
                    }

                    promise.complete(Ok(
                        nodes.unwrap_or_else(|| vec![])
                    ));
                }
            });

            unordered.push(future);
            let _ = server.lock().unwrap().send_call(call).map_err(|e| {
                warn!("{}", e);
            });
        };

        let mut nodes: Vec<NodeInfo> = Vec::new();
        while let Some(result) = unordered.next().await {
            if let Ok(items) = result {
                for item in items {
                    nodes.push(item);
                }
            }
        }

        let (entry_sz, bucket_sz) = {
            let locked = dht.lock().unwrap();
            let locked_rt = locked.rt().lock().unwrap();
            (locked_rt.number_of_entries(), locked_rt.size())
        };

        let mut ordered = FuturesOrdered::<
            Pin<Box<dyn Future<Output = Result<()>>>>
        >::new();

        // breadth-first lookup: fill more buckets
        if !nodes.is_empty() && entry_sz == 0 {
            let (promise, future) = Promise::<()>::pair();
            dht.lock().unwrap().fill_home_bucket(nodes, promise);
            ordered.push_back(Box::pin(future));
        };

        if bucket_sz > 1 {
            // depth-first lookup: fill each bucket
            // only if the routing table is more than 1 bucket
            let futures = dht.lock().unwrap().fill_buckets();
            let chained = async move {
                futures::future::join_all(futures).await;
                Ok(())
            };
            ordered.push_back(Box::pin(chained));
        }

        while let Some(result) = ordered.next().await {
            let _ = result;
        }

        {
            let mut locked = dht.lock().unwrap();
            locked.bootstrapping.store(false, Ordering::Relaxed);
            locked.last_bootstrap = SystemTime::now();
        };

        info!("DHT {}:{} bootstrapping finished", network, self_id);
    }

    fn add_bootstrap_nodes(&mut self, nodes: &[NodeInfo]) {
        let total = self.bootstrap_nodes.len() + nodes.len();
        let mut dedup = HashMap::<Id, NodeInfo>::with_capacity(total);

        let self_id = *self.id();
        for item in self.bootstrap_nodes.clone() {
            dedup.insert(*item.id(), item);
        }
        for item in nodes.to_vec() {
            if !self.network.can_use_address(item.socket_addr()) {
                continue;
            }
            if item.id() == &self_id {
                continue;
            }
            dedup.insert(*item.id(), item);
        }

        self.bootstrap_nodes = dedup.values().cloned().collect();
        self.bootstrap_ids   = dedup.keys().cloned().collect();
    }

    pub(crate) fn find_node(&self,
        target: &Id,
        option: LookupOption,
        promise: Promise<Option<NodeInfo>>
    ) {
        let node: Option<NodeInfo> = self.rt().lock().unwrap()
                .bucket_entry(target)
                .map(|v| v.into());

        if option == LookupOption::Local {
            promise.complete(Ok(None));
            return;
        }
        if option == LookupOption::Conservative && node.is_some() {
            promise.complete(Ok(node));
            return;
        }

        let mut task = Box::new(NodeLookupTask::new(
            self.dht(),
            target.clone(),
            option != LookupOption::Conservative
        ));
        task.with_name(format!("Lookup node: {target}"));
        task.with_want_target(true);
        task.with_listener(
            TaskListener::default().ended_fn(
                move |t: &dyn Task| {
                    let task = t.as_any()
                        .downcast_ref::<NodeLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            })
        );

        self.task_man.add(task);
    }


    pub(crate) fn find_value(
        &self,
        value_id: &Id,
        expected_seq: i32,
        option: LookupOption,
        promise: Promise<Option<Value>>
    ) {
        let mut task = Box::new(ValueLookupTask::new(
            self.dht(),
            value_id.clone(),
            expected_seq,
            option != LookupOption::Conservative
        ));
        task.with_name(format!("Lookup value: {value_id}"));
        task.with_listener(
            TaskListener::default().ended_fn(
                move |t: &dyn Task| {
                    let task = t.as_any()
                        .downcast_ref::<ValueLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            })
        );

        self.task_man.add(task);
    }

    pub(crate) fn store_value(
        &self,
        value: Value,
        expected_seq: i32,
        promise: Promise::<()>
    ) {
        let valueid = value.id();
        let mut nested = Box::new(ValueAnnounceTask::new(
            self.dht(), value.clone(), expected_seq
        ));
        nested.with_name(format!("Store value:{valueid}"));
        nested.with_listener(
            TaskListener::default().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        let task_man = self.task_man.clone();
        // Lookup task to find the closest nodes to the valueid, and
        // then nested announce task to announce the value to those nodes.
        let mut task = Box::new(NodeLookupTask::new(
            self.dht(), valueid, false
        ));
        task.with_name(format!("Store value: lookup closest node to {valueid}"));
        task.with_want_token(true);
        task.with_nested(nested);
        task.with_listener({
            TaskListener::default().ended_fn({
                let task_man = task_man.clone();
                move |t: &dyn Task| {
                    let task = t.as_any()
                        .downcast_ref::<NodeLookupTask>().unwrap();

                    if task.task_state() != State::Completed {
                        return;
                    }
                    let Some(mut nested) = task.nested() else {
                        return;
                    };

                    let closest = task.closest();
                    if closest.is_empty() {
                        // This should never happen
                        warn!("!!! Store value task not started because the node lookup task got the empty closest nodes.");
                        nested.cancel();
                        return;
                    }

                    nested.as_any()
                        .downcast_ref::<ValueAnnounceTask>().unwrap()
                        .with_closest(closest.clone());

                    task_man.add(nested);
            }})
        });

        task_man.add(task);
    }

    pub(crate) fn find_peer(
        &self,
        peerid: &Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption,
        promise: Promise::<Vec<PeerInfo>>
    ) {
        let mut task = Box::new(PeerLookupTask::new(
            self.dht(),
            peerid.clone(),
            expected_seq,
            expected_count,
            option != LookupOption::Conservative
        ));
        task.with_name(format!("Lookup peer: {peerid}"));
        task.with_listener({
            TaskListener::default().ended_fn(
                move |t: &dyn Task| {
                    let task = t.as_any()
                        .downcast_ref::<PeerLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            })
        });

        self.task_man.add(task);
    }

    pub(crate) fn announce_peer(
        &self,
        peer: PeerInfo,
        expected_seq: i32,
        promise: Promise::<()>
    ) {
        // Announce task to announce the peer to the closest nodes found
        // by the lookup task.
        let mut nested = Box::new(PeerAnnounceTask::new(
            self.dht(), peer.clone(), expected_seq,
        ));
        nested.with_name(format!("Announce peer: {}", peer.id()));
        nested.with_listener(
            TaskListener::default().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        let task_man = self.task_man.clone();
        // Lookup task to find the closest nodes to the targetid.
        let mut task = Box::new(NodeLookupTask::new(
            self.dht(), peer.id().clone(), false
        ));
        task.with_name(format!("Announce peer: lookup closest node to {}", peer.id()));
        task.with_want_token(true);
        task.with_nested(nested);
        task.with_listener({
            TaskListener::default().ended_fn({
                let task_man = task_man.clone();
                move |t: &dyn Task| {
                    let task = t.as_any()
                        .downcast_ref::<NodeLookupTask>().unwrap();

                    if task.task_state() != State::Completed {
                        return;
                    }
                    let Some(mut nested) = task.nested() else {
                        return;
                    };

                    let closest = task.closest();
                    if closest.is_empty() {
                        // This should never happen
                        warn!("!!! Announce peer task not started because the node lookup task got the empty closest nodes.");
                        nested.cancel();
                        return;
                    }

                    nested.as_any()
                        .downcast_ref::<PeerAnnounceTask>().unwrap()
                        .with_closest(closest.clone());

                    task_man.add(nested);

            }})
        });

        task_man.add(task);
    }
}
