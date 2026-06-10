use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::SocketAddr,
    sync::{Arc, Mutex, Weak},
    time::SystemTime,
    path::{PathBuf, Path},
};
use indexmap::map::IndexMap;
use tokio::{task::JoinHandle};
use futures::stream::{FuturesUnordered, StreamExt};
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
    promise::Promise,
    token_manager::TokenManager,
    lookup_option::LookupOption,
    consumer::Consumer,
    timer_client::TimerClient,
    storage::data_storage::DataStorage,
    rpc::{
        Reachability,
        RpcCall, rpccall::State as CallState,
        rpc_server::{self, RpcServer},
        rpc_target::NodeInfoLike,
    },
    msg::{
        Message,
        LookupRequest, LookupResponse,
        msg::{*, Kind, Method, Body},
    },
    suspicious_node_detector::{
        SuspiciousNodeDetector,
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
    storage         : Option<Arc<Mutex<Box<dyn DataStorage>>>>,
    tokenman        : Option<Arc<TokenManager>>,
    timer_client    : Option<Arc<TimerClient>>,
    data_dir        : Option<&'a Path>,
    bootstrap_nodes : Option<&'a[NodeInfo]>,
}

impl<'a> Builder<'a> {
    pub(crate) fn with_identity(&mut self, identity: Arc<CryptoIdentity>) -> &mut Self {
        self.identity = Some(identity);
        self
    }

    pub(crate) fn with_bootstrap_nodes(&mut self, bootstrap_nodes: &'a [NodeInfo]) -> &mut Self {
        self.bootstrap_nodes = Some(bootstrap_nodes);
        self
    }

    pub(crate) fn with_timer_client(&mut self, timer_client: Arc<TimerClient>) -> &mut Self {
        self.timer_client = Some(timer_client);
        self
    }

    pub(crate) fn with_storage(&mut self, storage: Arc<Mutex<Box<dyn DataStorage>>>) -> &mut Self {
        self.storage = Some(storage);
        self
    }

    pub(crate) fn with_tokenman(&mut self, tokenman: Arc<TokenManager>) -> &mut Self {
        self.tokenman = Some(tokenman);
        self
    }

    pub(crate) fn with_datadir(&mut self, datadir: &'a Path) -> &mut Self {
        self.data_dir = Some(datadir);
        self
    }

    pub(crate) fn build_dht4(&self, host: &str, port: u16) -> Result<DHT> {
        let persist_file = self.data_dir
            .as_ref()
            .expect("No data directory specified")
            .join("dht4.cache");

        DHT::new(self, Network::IPv4, host, port, persist_file.as_path())
    }

    pub(crate) fn build_dht6(&self, host: &str, port: u16) -> Result<DHT> {
        let persist_file = self.data_dir
            .as_ref()
            .expect("No data directory specified")
            .join("dht6.cache");

        DHT::new(self, Network::IPv6, host, port, persist_file.as_path())
    }
}

pub(crate) struct DHT {
    identity        : Arc<CryptoIdentity>,
    ni              : Arc<NodeInfo>,
    network         : Network,
    host            : String,
    port            : u16,
    is_running      : bool,
    status          : ConnectionStatus,

    storage         : Arc<Mutex<Box<dyn DataStorage>>>,
    tokenman        : Arc<TokenManager>,
    taskman         : Arc<TaskManager>,
    rpc_server      : Option<Arc<Mutex<RpcServer>>>,

    persist_file    : PathBuf,
    last_saved      : Option<SystemTime>,
    rt              : Arc<Mutex<RoutingTable>>,

    bootstrap_nodes : Vec<NodeInfo>,
    bootstrap_ids   : Vec<Id>,
    last_bootstrap  : SystemTime,
    bootstrapping   : bool,

    last_maintenance: SystemTime,
    maintenance_tasks: HashSet<Prefix>,

    timer_client    : Arc<TimerClient>,

    suspicious_detector         : Option<Arc<Mutex<dyn SuspiciousNodeDetector>>>,
    pub(crate) weak : Weak<Mutex<DHT>>,

    quit_flag       : Arc<Mutex<bool>>,
    server_task     : Option<JoinHandle<()>>,
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
        builder : &Builder,
        network : Network,
        host    : &str,
        port    : u16,
        persist_file: &Path,
    ) -> Result<Self> {
        assert!(builder.identity.is_some());
        assert!(builder.storage.is_some());
        assert!(builder.timer_client.is_some());
        assert!(builder.tokenman.is_some());
        //assert!(builder.bootstrap_nodes.is_some());
        assert!(builder.data_dir.is_some());

        let identity = builder.identity.as_ref().unwrap().clone();
        let storage  = builder.storage.as_ref().unwrap().clone();
        let tokenman = builder.tokenman.as_ref().unwrap().clone();
        let tclient  = builder.timer_client.as_ref().unwrap().clone();

        let nodeid = identity.id().clone();
        let host   = host.to_string();
        let socket_addr = SocketAddr::new(host.parse()?, port);
        let ni = Arc::new(NodeInfo::new(nodeid, socket_addr));
        let bootstrap_nodes = builder.bootstrap_nodes
            .map(|nodes| nodes.to_vec())
            .unwrap_or_else(Vec::new);

        Ok( Self {
            identity,
            network,
            host,
            port,
            ni,
            is_running      : false,
            status          : ConnectionStatus::Disconnected,
            storage,
            tokenman,
            taskman         : Arc::new(TaskManager::new()),
            rpc_server      : None,
            rt              : Arc::new(Mutex::new(RoutingTable::new(nodeid))),
            persist_file    : persist_file.to_path_buf(),
            last_saved      : None,
            bootstrap_nodes,
            bootstrap_ids   : Vec::new(),
            last_bootstrap  : SystemTime::UNIX_EPOCH,
            last_maintenance: SystemTime::UNIX_EPOCH,
            maintenance_tasks: HashSet::new(),
            bootstrapping   : false,
            timer_client    : tclient,
            suspicious_detector: None,
            weak            : Weak::new(), // will be set later
            quit_flag       : Arc::new(Mutex::new(false)),
            server_task     : None,
        })
    }

    pub(crate) fn network(&self) -> Network {
        self.network
    }

    pub(crate) fn ni(&self) -> Arc<NodeInfo> {
        self.ni.clone()
    }

    pub(crate) fn id(&self) -> &Id {
        self.ni.id()
    }

    pub(crate) fn addr(&self) -> &SocketAddr {
        self.ni.socket_addr()
    }

    pub(crate) fn rpc_server(&self) -> Arc<Mutex<RpcServer>> {
        self.rpc_server.as_ref()
            .expect("RpcServer not initialized")
            .clone()
    }

    fn send_msg(&self, msg: &mut Message) {
        let _ = self.rpc_server.as_ref().expect("Rpc server not initalized")
                    .lock().unwrap()
                    .send_msg(msg)
                    .map_err(|e| error!("{e}"))
                    .map(|_|());
    }

    pub(crate) fn send_call(&self, call: RpcCall) {
        let _ = self.rpc_server.as_ref().expect("Rpc server not initalized")
                    .lock().unwrap()
                    .send_call(call)
                    .map_err(|e| error!("{e}"))
                    .map(|_|());
    }

    pub(crate) fn rt(&self) -> Arc<Mutex<RoutingTable>> {
        self.rt.clone()
    }

    fn fill_home_bucket(dht: Arc<Mutex<DHT>>, nodes: Vec<NodeInfo>) -> impl Future<Output = Result<()>> {
        let promise = Promise::<()>::new();
        let future = promise.future();

        if nodes.is_empty() {
            promise.complete(Ok(()));
            return future;
        }

        let (weak, nodeid, taskman) = {
            let locked = dht.lock().unwrap();
            (locked.weak.clone(), locked.id().clone(), locked.taskman.clone())
        };
        let mut task = Box::new(NodeLookupTask::new(
            weak,
            nodeid,
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

        taskman.add(task);
        return future;
    }

    fn fill_buckets(dht: Arc<Mutex<DHT>>) -> impl Future<Output = Result<()>> {
        let (number_of_entries, buckets, weak_dht, taskman) = {
            let locked_dht = dht.lock().unwrap();
            let locked_rt  = locked_dht.rt.lock().unwrap();
            (
                locked_rt.number_of_entries(),
                locked_rt.buckets(),
                locked_dht.weak.clone(),
                locked_dht.taskman.clone(),
            )
        };

        let mut futures_unordered = FuturesUnordered::new();
        for bucket in buckets {
            if bucket.lock().unwrap().is_full() &&
                number_of_entries >= Self::BOOTSTRAP_IF_LESS_THAN_X_ENTRIES {
                continue;
            }

            let (lookup_target, bucket_prefix) = {
                let mut locked = bucket.lock().unwrap();
                locked.update_refresh_time();
                (locked.prefix().random_id(), locked.prefix().clone())
            };

            let promise = Promise::<()>::new();
            let future = promise.future();

            let mut task = Box::new(NodeLookupTask::new(
                weak_dht.clone(),
                lookup_target,
                false,
            ));
            task.with_name(format!("Bootstrap: filling Bucket - {}", bucket_prefix));
            task.with_listener(
                TaskListener::default().ended_fn(
                    move |_| promise.complete(Ok(()))
                )
            );
            taskman.add(task);
            futures_unordered.push(future);
        }

        return async move {
            while let Some(result) = futures_unordered.next().await {
                result?;
            }
            Ok(())
        }
    }

    fn try_ping_maintenance(&mut self,
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

        if self.maintenance_tasks.contains(&prefix) {
            return;
        }

        if need_refresh || need_replacement  {
            let mut task = Box::new(PingRefreshTask::new(self.weak.clone()));
            task.with_name(name);
            task.with_check_all(check_all);
            task.with_remove_on_timeout(remove_on_timeout);
            task.with_bucket(bucket);

            if self.maintenance_tasks.insert(prefix) {
                let weak = self.weak.clone();
                task.with_listener(TaskListener::default().ended_fn(move |_| {
                    if let Some(dht) = weak.upgrade() {
                        dht.lock().unwrap().maintenance_tasks.remove(&prefix);
                    }
                }));
                self.taskman.add(task);
            }
        }
    }

    pub(crate) fn random_lookup(&mut self) {
        if !self.rpc_server().lock().unwrap().is_reachable() {
            debug!("Periodic: not performing random lookup, server is unreachable");
            return;
        }

        let mut task = Box::new(NodeLookupTask::new(
            self.weak.clone(),
            Id::random(),
            false,
        ));
        task.with_name("Periodic: random node lookup".into());
        self.taskman.add(task);
    }

    pub(crate) fn random_ping(&mut self) {
        let (is_reachable, has_pending) = {
            let server = self.rpc_server();
            let locked = server.lock().unwrap();

            (
                locked.is_reachable(),
                locked.has_pending_calls()
            )
        };

        if !is_reachable {
            info!("Periodic: not performing random ping, server is unreachable");
            return;
        }
        if has_pending {
            info!("Periodic: not performing random ping, server has pending calls.");
            return;
        }


        let Some(entry) = ({
            let locked_rt = self.rt.lock().unwrap();
            locked_rt.random_entry()
        }) else {
            return;
        };

        info!("Periodic: random ping ...");

        let call = RpcCall::new(entry, ping_request());
        let _ = self.send_call(call);
    }

    fn update(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Periodic: DHT update...");
        self.rt_maintenance();

        let entries = self.rt.lock().unwrap().number_of_entries();
        if entries < Self::BOOTSTRAP_IF_LESS_THAN_X_ENTRIES ||
            crate::elapsed_ms!(self.last_bootstrap) > Self::SELF_LOOKUP_INTERVAL {

            let bootstrap_nodes = if entries < Self::USE_BOOTSTRAP_NODES_IF_LESS_THAN_X_ENTRIES {
                self.bootstrap_nodes.clone()
            } else {
                Vec::new()
            };

            let weak = self.weak.clone();
            let _ = tokio::spawn(async move {
                if let Some(dht) = weak.upgrade() {
                    DHT::do_bootstrap(dht, bootstrap_nodes).await;
                }
            });
        }
    }

    fn rt_maintenance(&mut self) {
        if self.last_maintenance.elapsed().map_or(true, |v| {
            v.as_millis() > Self::ROUTING_TABLE_MAINTENANCE_INTERVAL
        }) {
            return;
        }

        info!("Routing table maintenance ...");
        self.last_maintenance = SystemTime::now();

        let weak_dht = self.weak.clone();
        self.rt.lock().unwrap().maintenance(
            self.bootstrap_ids.as_ref(),
            Consumer::new(move |bucket: Arc<Mutex<KBucket>>| {
                let Some(dht) = weak_dht.upgrade() else{
                    panic!("DHT instance is dropped");
                };

                let prefix = bucket.lock().unwrap().prefix().clone();
                dht.lock().unwrap().try_ping_maintenance(bucket, false, false, false,
                    format!("Routing table maintenance: refreshing bucket {}", prefix)
                );
            })
        );
    }

    fn persist_routing_table(&mut self) {
        info!("Periodic: persisting routing table ...");
        match self.rt.lock().unwrap().save(self.persist_file.as_path()) {
            Ok(()) => self.last_saved = Some(SystemTime::now()),
            Err(err) => error!("Can not save the routing table: {}", err),
        }
    }

    fn purge_suspicious_nodes(&mut self) {
        if let Some(detector) = self.suspicious_detector.as_ref() {
            info!("Periodic: purging suspicious nodes ...");
            detector.lock().unwrap().purge();
        }
    }

    fn set_status(&mut self, status: ConnectionStatus) {
        if self.status == status {
            return;
        }

        let old = self.status;
        self.status = status;

        info!("DHT {}:{} connection status changed: {} -> {}",
            self.network, self.id(), old, self.status
        );

        // TODO:
    }

    pub(crate) fn set_connection_status_listener(&mut self) {
        // TODO: unimplemented!();
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        info!("Starting DHT/{}:{} on {} ...", self.network, self.id(), self.addr());

        let path = Path::new(self.persist_file.as_path());
        if path.exists() || path.is_file() {
            debug!("Loading routing table from {}.", path.display());
            let file = path.display().to_string();
            let _ = self.rt.lock().unwrap().load(&path)
                .map(|_| debug!("Loaded routing table from {}.", file))
                .map_err(|e| warn!("Failed to load routing table from {}: {}", file, e));
        }

        // initialize RPC server
        let mut server = RpcServer::new(
            self.ni(),
            self.identity.clone(),
            self.suspicious_detector.clone()
        );
        server.message_handler({
            let weak = self.weak.clone();
            move |msg| {
                weak.upgrade()
                    .expect("DHT instance is dropped")
                    .lock().unwrap()
                    .on_message(msg)
            }
        });
        server.callsent_handler({
            let rt = self.rt();
            move |call| {
                let nodeid = call.target_id();
                rt.lock().unwrap().on_request_sent(&nodeid);
            }
        });
        server.calltimeout_handler({
            let rt = self.rt();
            move |call| {
                let nodeid = call.target_id();
                rt.lock().unwrap().on_timeout(&nodeid);
            }
        });
        server.start().await?;
        server.reachable_handler({
            let weak = self.weak.clone();
            move |reachable| {
                let dht = weak.upgrade().expect("DHT instance is dropped");
                let mut locked = dht.lock().unwrap();

                if reachable {
                    locked.set_status(ConnectionStatus::Connected);
                } else {
                    locked.random_ping();
                    locked.set_status(ConnectionStatus::Disconnected);
                }
            }
        });

        self.rpc_server = Some(Arc::new(Mutex::new(server)));
        self.set_status(ConnectionStatus::Connecting);

        let server = self.rpc_server();
        let quit   = self.quit_flag.clone();
        let task   = tokio::task::spawn_blocking(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build().expect("no rpc server engine runtime build")
                .block_on(async move {
                    rpc_server::run_loop(server, quit).await;
                }
            );
        });

        self.setup_periodic_tasks().await?;
        self.server_task = Some(task);
        self.is_running = true;

        info!("Started DHT/{}:{} on {}:{}", self.network, self.id(), self.host, self.port);
        Ok(())
    }

    pub(crate) async fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Stopping DHT {}:{} on {}:{}......",
            self.network, self.id(), self.host, self.port);

        self.is_running = false;
        self.bootstrapping = false;
        self.set_status(ConnectionStatus::Disconnected);

        self.rpc_server.take().map(async |s| s.lock().unwrap().stop().await);
        self.server_task.take().map(async |t| t.await);
        self.taskman.stop();
        self.rt.lock().unwrap().save(&self.persist_file).map_err(|e| {
            warn!("Failed to persist routing table to {}: {}", self.persist_file.display(), e);
        }).ok();
        self.last_saved = Some(SystemTime::now());

        info!("Stopped DHT {}:{} on {}:{}.",
            self.network, self.id(), self.host, self.port);
    }

    async fn setup_periodic_tasks(&self) -> Result<()> {
        let weak = self.weak.clone();
        let _ = self.timer_client.add_timer(30, Some(320),
            move || {
                weak.upgrade().expect("DHT instance is dropped")
                    .lock().unwrap()
                    .update()
            }
        ).await?;

        let weak = self.weak.clone();
        let _ = self.timer_client.add_timer(
            Self::RANDOM_LOOKUP_INTERVAL,
            Some(Self::RANDOM_LOOKUP_INTERVAL),
            move || {
                weak.upgrade().expect("DHT instance is dropped")
                    .lock().unwrap()
                    .random_lookup();
            }
        ).await?;

        let weak = self.weak.clone();
        let _ = self.timer_client.add_timer(
            Self::RANDOM_PING_INTERVAL,
            Some(Self::RANDOM_PING_INTERVAL),
            move || {
                weak.upgrade().expect("DHT instance is dropped")
                    .lock().unwrap()
                    .random_ping();
            }
        ).await?;

        if self.suspicious_detector.is_some() {
            let weak = self.weak.clone();
            self.timer_client.add_timer(60, Some(30),
                move || {
                    weak.upgrade().expect("DHT instance is dropped")
                        .lock().unwrap()
                        .purge_suspicious_nodes()
                }
            ).await?;
        };

        let weak = self.weak.clone();
        self.timer_client.add_timer(
            120,
            Some(Self::ROUTING_TABLE_PERSIST_INTERVAL),
            move || {
                weak.upgrade().expect("DHT instance is dropped")
                    .lock().unwrap()
                    .persist_routing_table();
            }
        ).await?;
        Ok(())
    }

    fn received(&mut self, msg: &Message) {
        let inconsistent_suspicious = |addr: SocketAddr, id: Id| {
            warn!("Received a message from inconsistent node {}@{}, ignored the potential
                  routing table update", id, addr);

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

        if let Some(known_id) = last_known_id(remote_addr) {
            if known_id != remote_id {

                // We already know a node with that address but with a different ID.
                // This might happen if one node changes its ID.
                // Force remove from the routing table to prevent suspicious behavior
                warn!("Received a message from suspicious node {}@{}, force-removing routing table entries because ID-change was detected; new ID {}",
                    remote_id, remote_addr, known_id);

                let mut locked_rt = self.rt.lock().unwrap();
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
                    //self.try_ping_maintenance(bucket.clone(), true, false, false,
                    //    format!("Checking bucket {} after ID change was detected", prefix));
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
                    //self.try_ping_maintenance(bucket.clone(), true, false, false,
                    //    format!("Checking bucket {} after ID change was detected", prefix));
                }

                inconsistent_suspicious(remote_addr, remote_id);
                return;
            }
        }

        let existing_opt = {
            let locked = self.rt.lock().unwrap();
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
            new_entry.update_last_sent(locked.sent_time());
        }

        self.rt.lock().unwrap().put(new_entry.clone());

        // Optimize: not the standard Kademlia behavior
		// incoming request && the new entry is unreachable && the target bucket not full,
		// then try to do a ping request to the new entry check its availability.
        if existing_opt.is_none() && !new_entry.is_reachable(){
            println!(">>> existing_opt.is_none():{}, reacable: {}",
                existing_opt.is_none(),
                new_entry.is_reachable()
            );
            // Verify the node, speed up the bootstrap process or make the bucket more reliable.
			// only if the new entry is unreachable and the bucket is not full yet
            let call = RpcCall::new(new_entry, ping_request());
            let _ = self.send_call(call);
        }
    }

    fn send_err(&mut self, method: Method, code: i32, str: &str) {
        let mut msg = error_msg(method, 0, code, str.into());
        // TODO: set remote id and addr
        self.send_msg(&mut msg);
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
        debug!("Received a response message {} from {}/{}, txid {}",
            msg.method(),
            msg.remote_addr(),
            msg.remote_id(),
            msg.txid()
        );
    }

    fn on_error(&mut self, msg: &Message) {
        let Some(Body::Error(err)) = msg.body() else {
            panic!("Panic: should be error message");
        };

        warn!("Error message from {}/{} - {}:{}, txid {}",
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
            panic!("Panic: ping request should not have body");
        }

        let txid = req.txid();
        let remote_id   = req.remote_id().clone();
        let remote_addr = req.remote_addr().clone();

        let mut msg = ping_response(txid);
        msg.set_remote(remote_id, remote_addr);

        self.send_msg(&mut msg);
    }

    fn fill_closest_nodes(&self, target: Id) -> Vec<NodeInfo> {
        let locked = self.rt.lock().unwrap();
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
            error!("Error: should be find node request");
            return;
        };

        let mut nodes4= None;
        let mut nodes6= None;
        let mut token = 0;

        let use_ipv4 = self.network().is_ipv4();
        let use_ipv6 = self.network().is_ipv6();
        let target = body.target().clone();

        if body.want4() && use_ipv4 {
            nodes4 = Some(self.fill_closest_nodes(target));
        }

        if body.want6() && use_ipv6 {
            nodes6 = Some(self.fill_closest_nodes(target));
        }
        if body.want_token() {
            token = self.tokenman.generate_token(
                req.nodeid(),
                req.remote_addr(),
                &target
            );
        }

        let txid = req.txid();
        let mut rsp = find_node_response(txid, nodes4, nodes6, token);
        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );
        rsp.set_nodeid(self.id().clone());

        self.send_msg(&mut rsp);
    }

    fn on_find_value(&mut self, req: &Message) {
        let Some(Body::FindValueRequest(body)) = req.body() else {
            error!("Error: should be find value request");
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
        let target = body.target().clone();

        let mut rsp = if value.is_none() {
            let use_ipv4 = self.network().is_ipv4();
            let use_ipv6 = self.network().is_ipv6();
            let mut nodes4 = None;
            let mut nodes6 = None;

            if body.want4() && use_ipv4 {
                nodes4 = Some(self.fill_closest_nodes(target));
            }

            if body.want6() && use_ipv6 {
                nodes6 = Some(self.fill_closest_nodes(target));
            }
            find_value_response_with_nodes(txid, nodes4, nodes6)
        } else {
            find_value_response(txid, value.unwrap())
        };
        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );

        self.send_msg(&mut rsp);
    }

    fn on_store_value(&mut self, req: &Message) {
        let Some(Body::StoreValueRequest(body)) = req.body() else {
            error!("Error: should be store value request");
            return;
        };

        let value = body.value();
        let value_id = value.id();
        let remote_addr = req.remote_addr().clone();

        let is_valid = self.tokenman.verify_token(
            body.token(),
            req.nodeid(),
            &remote_addr,
            &value_id
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
                self.send_err(Method::StoreValue, 300, "Cannot replace mismatched mutable/immutable value");
                return;
            }
            if value.sequence_number() < existing.sequence_number() {
                warn!("Rejecting value {}: sequence number {} is less than existing {}", value_id, value.sequence_number(), existing.sequence_number());
                self.send_err(Method::StoreValue, 300, "Sequence number is less than existing value");
                return;
            }
            if body.expected_seq() >= 0 && existing.sequence_number() > body.expected_seq() {
                warn!("Rejecting value {}: existing sequence number {} is greater than expected {}", value_id, existing.sequence_number(), body.expected_seq());
                self.send_err(Method::StoreValue, 300, "Existing sequence number is greater than expected");
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

        let txid = req.txid();
        let mut msg = store_value_response(txid);
        msg.set_remote(
            req.remote_id().clone(),
            remote_addr
        );

        self.send_msg(&mut msg);
    }

    fn on_find_peer(&mut self, req: &Message) {
        let Some(Body::FindPeerRequest(body)) = req.body() else {
            error!("Panic: should be find peer request");
            return;
        };

        let result = self.storage.lock().unwrap().get_peers_with_expected_seq(
            body.target(),
            body.expected_seq(),
            body.expected_count()
        );
        let peers = match result {
            Ok(v) => v,
            Err(e) => {
                warn!("Retrieve peers for {} error: {}", body.target(), e);
                return;
            }
        };

        let txid = req.txid();
        let target = body.target().clone();

        let mut rsp = if peers.is_empty() {
            let use_ipv4 = self.network().is_ipv4();
            let use_ipv6 = self.network().is_ipv6();
            let mut nodes4 = None;
            let mut nodes6 = None;

            if body.want4() && use_ipv4 {
                nodes4 = Some(self.fill_closest_nodes(target));
            }
            if body.want6() && use_ipv6 {
                nodes6 = Some(self.fill_closest_nodes(target));
            }
            find_peer_response_with_nodes(txid, nodes4, nodes6)
        } else {
            find_peer_response(txid, peers)
        };

        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );
        self.send_msg(&mut rsp);
    }

    fn on_announce_peer(&mut self, req: &Message) {
        let Some(Body::AnnouncePeerRequest(body)) = req.body() else {
            error!("Panic: should be announce peer request");
            return;
        };

        let peer = body.peer();
        let remote_addr = req.remote_addr().clone();
        let is_valid = self.tokenman.verify_token(
            body.token(),
            req.nodeid(),
            &remote_addr,
            peer.id()
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
                self.send_err(Method::AnnouncePeer, 300, "Sequence number is less than existing value");
                return;
            }

            if body.expected_seq() >= 0 && existing.sequence_number() > body.expected_seq() {
                warn!("Rejecting peer {}: existing sequence number {} is greater than expected {}", peer.id(), existing.sequence_number(), body.expected_seq());
                self.send_err(Method::AnnouncePeer, 300, "Existing sequence number is greater than expected");
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

        let txid = req.txid();
        let mut msg = announce_peer_response(txid);
        msg.set_remote(
            req.remote_id().clone(),
            remote_addr
        );

        self.send_msg(&mut msg);
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
        if locked.bootstrapping {
            warn!("The DHT/{} instance is already bootstrapping.", locked.network);
            return;
        }

        locked.last_bootstrap = SystemTime::UNIX_EPOCH;
        // Todo: handle status.

        drop(locked);

        DHT::do_bootstrap(dht, nodes).await;
    }

    async fn do_bootstrap(
        dht: Arc<Mutex<Self>>,
        bootstrap_nodes: Vec<NodeInfo>,
    ) {
        if bootstrap_nodes.is_empty() {
            return;
        }

        let (network, server) = {
            let mut locked = dht.lock().unwrap();
            if locked.bootstrapping {
                return;
            }
            if crate::elapsed_ms!(locked.last_bootstrap) < Self::BOOTSTRAP_MIN_INTERVAL as u128 {
                return;
            }
            if bootstrap_nodes.is_empty() && locked.rt.lock().unwrap().is_empty() {
                warn!("no bootstrap nodes provided and routing table is empty.");
                return;
            }

            locked.bootstrapping = true;

            let network = locked.network();
            info!("DHT/{}:{} bootstrapping ...", network, locked.id());

            (network, locked.rpc_server())
        };

        let mut futures = FuturesUnordered::new();

        for item in bootstrap_nodes.into_iter() {
            let msg = find_node_request(
                Id::random(),
                network.is_ipv4(),
                network.is_ipv6(),
                Some(true)
            );

            let mut call = RpcCall::new(item, msg);
            let promise = Promise::<Vec<NodeInfo>>::new();
            let future  = promise.future();

            call.set_simple_listener(move |_call, _, cur| {
                if cur.is_final() {
                    let mut nodes = None;

                    if cur == CallState::Responded {
                        let Some(rsp) = _call.rsp() else {
                            promise.complete(Ok([].to_vec()));
                            return;
                        };
                        let Some(body) = rsp.body() else {
                            promise.complete(Ok([].to_vec()));
                            return;
                        };
                        let Body::FindNodeResponse(body) = body else {
                            promise.complete(Ok([].to_vec()));
                            return;
                        };
                        nodes = body.nodes(network).map(|v| v.to_vec());
                    }

                    promise.complete(Ok(
                        nodes.unwrap_or_else(|| [].to_vec())
                    ));
                }
            });

            let _ = server.lock().unwrap().send_call(call).map_err(|e| {
                warn!("{}", e);
            });

            futures.push(async move {
                match future.clone().await {
                    Ok(_) => future.result(),
                    Err(e) => Err(e),
                }
            });
        };

        let mut nodes = HashMap::<Id, NodeInfo>::new();
        while let Some(result) = futures.next().await {
            if let Ok(items) = result {
                for item in items {
                    nodes.insert(item.id().clone(), item.clone());
                }
            }
        }

        let nodes: Vec<NodeInfo> = nodes.into_values().collect();

        let (has_rt_entries, rt_size) = {
            let locked = dht.lock().unwrap();
            let locked_rt = locked.rt.lock().unwrap();
            (!locked_rt.is_empty(), locked_rt.size())
        };

        if !nodes.is_empty() && has_rt_entries {
            _ = Self::fill_home_bucket(dht.clone(), nodes).await;
        };

        if rt_size > 1 {
            _ = Self::fill_buckets(dht.clone()).await;
        }

        let nodeid = {
            let mut locked = dht.lock().unwrap();
            locked.bootstrapping = false;
            locked.last_bootstrap = SystemTime::now();
            locked.id().clone()
        };

        info!("DHT {}:{} bootstrapping finished", network, nodeid);
    }

    fn add_bootstrap_nodes(&mut self, nodes: &[NodeInfo]) {
        let capacity = self.bootstrap_nodes.len() + nodes.len();
        let mut dedup = IndexMap::<Id, NodeInfo>::with_capacity(capacity);

        while let Some(ni) = self.bootstrap_nodes.pop() {
            dedup.insert(ni.id().clone(), ni);
        };

        let self_id = self.id().clone();
        for ni in nodes.iter() {
            if !self.network.can_use_address(ni.socket_addr()) {
                continue;
            }
            if &self_id == ni.id() {
                continue;
            }
            dedup.insert(ni.id().clone(), ni.clone());
        };

        self.bootstrap_nodes = dedup.values().cloned().collect();
        self.bootstrap_ids = dedup.keys().cloned().collect();
    }

    pub(crate) async fn find_node(
        &self,
        target: &Id,
        option: LookupOption
    ) -> Result<Option<NodeInfo>> {

        let node = self.rt.lock().unwrap().bucket_entry(target).map(|v| v.ni());
        if option == LookupOption::Local {
            return Ok(node);
        }
        if option == LookupOption::Conservative && node.is_some() {
            return Ok(node);
        }

        let promise = Promise::<Option<NodeInfo>>::new();
        let future  = promise.future();

        let mut task = Box::new(NodeLookupTask::new(
            self.weak.clone(),
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

        self.taskman.add(task);

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn find_value(
        &self,
        value_id: &Id,
        expected_seq: i32,
        option: LookupOption
    ) -> Result<Option<Value>> {

        let promise = Promise::<Option<Value>>::new();
        let future  = promise.future();

        let mut task = Box::new(ValueLookupTask::new(
            self.weak.clone(),
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

        self.taskman.add(task);

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn store_value(
        &self,
        value: Value,
        expected_seq: i32
    ) -> Result<()> {

        let taskman = self.taskman.clone();
        let promise = Promise::<()>::new();
        let future  = promise.future();
        let valueid = value.id();

        let mut nested = Box::new(ValueAnnounceTask::new(
            self.weak.clone(), value.clone(), expected_seq
        ));
        nested.with_name(format!("Store value:{valueid}"));
        nested.with_listener(
            TaskListener::default().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        // Lookup task to find the closest nodes to the valueid, and
        // then nested announce task to announce the value to those nodes.
        let mut task = Box::new(NodeLookupTask::new(
            self.weak.clone(), valueid, false
        ));
        task.with_name(format!("Store value: lookup closest node to {valueid}"));
        task.with_want_token(true);
        task.with_nested(nested);
        task.with_listener({
            TaskListener::default().ended_fn({
                let taskman = taskman.clone();
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

                    taskman.add(nested);
            }})
        });

        taskman.add(task);

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn find_peer(
        &self,
        peerid: &Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption
    ) -> Result<Vec<PeerInfo>> {

        let promise = Promise::<Vec<PeerInfo>>::new();
        let future  = promise.future();

        let mut task = Box::new(PeerLookupTask::new(
            self.weak.clone(),
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

        self.taskman.add(task);

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn announce_peer(
        &self,
        peer: PeerInfo,
        expected_seq: i32
    ) -> Result<()>{
        let taskman = self.taskman.clone();
        let promise = Promise::<()>::new();
        let future  = promise.future();

        // Announce task to announce the peer to the closest nodes found
        // by the lookup task.
        let mut nested = Box::new(PeerAnnounceTask::new(
            self.weak.clone(), peer.clone(), expected_seq
        ));
        nested.with_name(format!("Announce peer: {}", peer.id()));
        nested.with_listener(
            TaskListener::default().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        // Lookup task to find the closest nodes to the peer, and
        // then nested announce task to announce to those nodes.
        let mut task = Box::new(NodeLookupTask::new(
            self.weak.clone(), peer.id().clone(), false
        ));
        task.with_want_token(true);
        task.with_name(format!("AnnouncePeer: lookup closest node to {}", peer.id()));
        task.with_nested(nested);
        task.with_listener(
            TaskListener::default().ended_fn({
                let taskman = taskman.clone();
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
                        warn!("!!! Peer announce task not started because the node lookup task got the empty closest nodes.");
                        nested.cancel();
                        return;
                    }

                    nested.as_any()
                        .downcast_ref::<PeerAnnounceTask>().unwrap()
                        .with_closest(closest.clone());

                    taskman.add(nested);
            }})
        );

        taskman.add(task);

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }
}
