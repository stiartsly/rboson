use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::SocketAddr,
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime}
};
use indexmap::map::IndexMap;
use futures::stream::{FuturesUnordered, StreamExt};
use log::{debug, info, warn, error};

use crate::{
    Id, Network,
    NodeInfo, PeerInfo, Value,
    errors::Result,
    crypto_identity::CryptoIdentity
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
        rpc_server::{RpcServer},
    },
    msg::{
        Message,
        LookupRequest, LookupResponse,
        msg::{self, Kind, Method, Body},
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

pub(crate) struct Builder<'a> {
    identity    : Option<Arc<CryptoIdentity>>,
    storage     : Option<Arc<Mutex<Box<dyn DataStorage>>>>,
    tokenman    : Option<Arc<TokenManager>>,
    timer_client: Option<Arc<TimerClient>>,
    data_dir    : Option<&'a str>,

    bootstrap_nodes: Option<&'a[NodeInfo]>,
}

impl<'a> Builder<'a> {
    pub(crate) fn new() -> Self {
        Self {
            identity: None,
            storage : None,
            tokenman: None,
            data_dir: None,
            bootstrap_nodes : None,
            timer_client    : None,
        }
    }

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

    pub(crate) fn with_datadir(&mut self, datadir: &'a str) -> &mut Self {
        self.data_dir = Some(datadir);
        self
    }

    pub(crate) fn build_dht4(&self, host: &str, port: u16) -> Result<DHT> {
        let data_dir = format!(
            "{}/dht4.cache",
            self.data_dir.as_ref().unwrap()
        );
        DHT::new(self, Network::IPv4, host, port, data_dir)
    }

    pub(crate) fn build_dht6(&self, host: &str, port: u16) -> Result<DHT> {
         let data_dir = format!(
            "{}/dht6.cache",
            self.data_dir.as_ref().unwrap()
        );
        DHT::new(self, Network::IPv4, host, port, data_dir)
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
    server          : Option<Arc<Mutex<RpcServer>>>,

    persist_file    : String,
    last_saved      : Option<SystemTime>,
    rt              : RoutingTable,

    bootstrap_nodes : Vec<NodeInfo>,
    bootstrap_ids   : Vec<Id>,
    last_bootstrap  : SystemTime,
    bootstrapping   : bool,

    last_maintenance: SystemTime,
    maintenance_tasks: HashSet<Prefix>,

    timer_client    : Arc<TimerClient>,

    suspicious_detector         : Option<Arc<Mutex<dyn SuspiciousNodeDetector>>>,
    pub(crate) weak_cloned      : Weak<Mutex<DHT>>
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
        persist_file: String,
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
            server          : None,
            rt              : RoutingTable::new(nodeid),
            persist_file,
            last_saved      : None,
            bootstrap_nodes,
            bootstrap_ids   : Vec::new(),
            last_bootstrap  : SystemTime::UNIX_EPOCH,
            last_maintenance: SystemTime::UNIX_EPOCH,
            maintenance_tasks: HashSet::new(),
            bootstrapping   : false,
            timer_client    : tclient,
            suspicious_detector: None,
            weak_cloned      : Weak::new(), // will be set later
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

    pub(crate) fn server(&self) -> Arc<Mutex<RpcServer>> {
        self.server.as_ref()
            .expect("RpcServer not initialized")
            .clone()
    }

    fn weak_dht(&self) -> Weak<Mutex<DHT>> {
        self.weak_cloned.clone()
    }

    pub(crate) fn rt(&self) -> &RoutingTable {
        &self.rt
    }

    pub(crate) fn rt_mut(&mut self) -> &mut RoutingTable {
        &mut self.rt
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
            (locked.weak_dht(), locked.id().clone(), locked.taskman.clone())
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
            let locked = dht.lock().unwrap();
            (
                locked.rt.number_of_entries(),
                locked.rt.buckets(),
                locked.weak_dht(),
                locked.taskman.clone(),
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
        if !self.server().lock().unwrap().is_reachable() {
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
            let mut task = Box::new(PingRefreshTask::new(
                self.weak_dht())
            );
            task.with_name(name);
            task.with_check_all(check_all);
            task.with_remove_on_timeout(remove_on_timeout);
            task.with_bucket(bucket);

            if self.maintenance_tasks.insert(prefix) {
                let weak = self.weak_dht();
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
        if !self.server().lock().unwrap().is_reachable() {
            debug!("Periodic: not performing random lookup, server is unreachable");
            return;
        }

        let mut task = Box::new(NodeLookupTask::new(
            self.weak_cloned.clone(),
            Id::random(),
            false,
        ));
        task.with_name("Periodic: random node lookup".into());
        self.taskman.add(task);
    }

    pub(crate) fn random_ping(&mut self) {
        let (is_reachable, has_pending) = {
            let server = self.server();
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

        let Some(entry) = self.rt.bucket_entry(None) else {
            return;
        };

        info!("Periodic: random ping ...");

        let call = RpcCall::with_entry(entry, msg::ping_request());
        let _ = self.server().lock().unwrap()
            .send_call(call);
    }

    fn update(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Periodic: DHT update...");
        self.rt_maintenance();

        let entries = self.rt.number_of_entries();
        if entries < Self::BOOTSTRAP_IF_LESS_THAN_X_ENTRIES ||
            crate::elapsed_ms!(self.last_bootstrap) > Self::SELF_LOOKUP_INTERVAL {

            let bootstrap_nodes = if entries < Self::USE_BOOTSTRAP_NODES_IF_LESS_THAN_X_ENTRIES {
                self.bootstrap_nodes.clone()
            } else {
                Vec::new()
            };

            let weak = self.weak_dht();
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

        let weak = self.weak_dht();
        self.rt.maintenance(
            self.bootstrap_ids.as_ref(),
            Consumer::new(move |bucket: Arc<Mutex<KBucket>>| {
                let prefix = bucket.lock().unwrap().prefix().clone();
                let Some(dht) = weak.upgrade() else{
                    panic!("DHT instance is dropped");
                };
                dht.lock().unwrap().try_ping_maintenance(bucket, false, false, false,
                    format!("Routing table maintenance: refreshing bucket {}", prefix)
                );
            })
        );
    }

    fn persist_routing_table(&mut self) {
        info!("Periodic: persisting routing table ...");
        match self.rt.save(&self.persist_file) {
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

        // initialize routing table
        let mut rt = RoutingTable::new(self.id().clone());
        if let Err(e) = rt.load(&self.persist_file) {
            warn!("Failed to load routing table from {}:{}.", self.persist_file, e);
        };

        // initialize RPC server
        let identity = self.identity.clone();
        let mut server = RpcServer::new(
            self.ni(), identity, self.suspicious_detector.clone()
        );
        server.message_handler({
            let dht = self.weak_dht();
            move |msg| {
                if let Some(dht) = dht.upgrade() {
                    dht.lock().unwrap().on_message(msg)
                }
            }
        });
        server.callsent_handler({
            let dht = self.weak_dht();
            move |call| {
                if let Some(dht) = dht.upgrade() {
                    dht.lock().unwrap().on_send(call)
                }
            }
        });
        server.calltimeout_handler({
            let dht = self.weak_dht();
            move |call| {
                if let Some(dht) = dht.upgrade() {
                    dht.lock().unwrap().on_timeout(call)
                }
            }
        });
        server.reachable_handler({
            let dht = self.weak_dht();
            move |reachable| {
                let Some(dht) = dht.upgrade() else {
                    return;
                };

                let mut locked_dht = dht.lock().unwrap();
                if reachable {
                    locked_dht.set_status(ConnectionStatus::Connected);
                } else {
                    locked_dht.random_ping();
                    locked_dht.set_status(ConnectionStatus::Disconnected);
                }
            }
        });

        server.start().await?;

        let server = Arc::new(Mutex::new(server));
        let weak = self.weak_dht();

        println!("dht >>>> line: {}", line!());
        let _ = RpcServer::run_loop(server.clone(), weak);

        self.server = Some(server);
        self.set_status(ConnectionStatus::Connecting);

        self.setup_periodic_tasks().await?;
        self.is_running = true;

        info!("Started DHT {}:{} on {}:{}.", self.network, self.id(), self.host, self.port);
        Ok(())
    }

    pub(crate) async fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Stopping DHT {}:{} on {}:{}......", self.network, self.id(), self.host, self.port);

        self.is_running = false;
        self.bootstrapping = false;
        self.set_status(ConnectionStatus::Disconnected);

        if let Some(server) = self.server.as_mut() {
            let mut locked = server.lock().unwrap();
            locked.reachable_handler(|_| {});
            let _ = locked.stop();
        }

        self.taskman.cancel_all();

        _ = self.rt.save(&self.persist_file).map_err(|err| {
            warn!("Failed to persist routing table to {}: {}", self.persist_file, err);
        });
        self.last_saved = Some(SystemTime::now());

        self.server = None;
        info!("Stopped DHT {}:{} on {}:{}.", self.network, self.id(), self.host, self.port);
    }

    async fn setup_periodic_tasks(&self) -> Result<()> {
        let weak = self.weak_dht();
        let _ = self.timer_client.add_timer(
            Duration::from_secs(30),
            Some(Duration::from_secs(320)),
            move || {
                if let Some(dht) = weak.upgrade() {
                    dht.lock().unwrap().update()
                }
            }
        ).await?;

        let weak = self.weak_dht();
        let _ = self.timer_client.add_timer(
            Duration::from_millis(Self::RANDOM_LOOKUP_INTERVAL),
            Some(Duration::from_millis(Self::RANDOM_LOOKUP_INTERVAL)),
            move || {
                if let Some(dht) = weak.upgrade() {
                    dht.lock().unwrap().random_lookup()
                }
            }
        ).await?;

        let weak = self.weak_dht();
        let _ = self.timer_client.add_timer(
            Duration::from_millis(Self::RANDOM_PING_INTERVAL),
            Some(Duration::from_millis(Self::RANDOM_PING_INTERVAL)),
            move || {
                if let Some(dht) = weak.upgrade() {
                    dht.lock().unwrap().random_ping()
                }
            }
        ).await?;

        let weak = self.weak_dht();
        self.timer_client.add_timer_if(
            self.suspicious_detector.is_some(),
            Duration::from_secs(60),
            Some(Duration::from_secs(30)),
            move || {
                if let Some(dht) = weak.upgrade() {
                    dht.lock().unwrap().purge_suspicious_nodes()
                }
            }
        ).await?;

        let weak = self.weak_dht();
        self.timer_client.add_timer(
            Duration::from_secs(120),
            Some(Duration::from_millis(Self::ROUTING_TABLE_PERSIST_INTERVAL)),
            move || {
                if let Some(dht) = weak.upgrade() {
                    dht.lock().unwrap().persist_routing_table()
                }
            }
        ).await?;
        Ok(())
    }

    fn received(&mut self, msg: &Message) {
        let from_addr  = msg.remote_addr().clone();

        let bogon_addr = if cfg!(feature = "devp") {
            !is_any_unicast(&from_addr.ip())
        } else {
            is_bogon(&from_addr)
        };

        if bogon_addr {
            info!("Received a message from bogon address {}, ignored the potential
                  routing table operation", from_addr);
            return;
        }

        let remote_id   = msg.nodeid();
        let remote_addr = msg.remote_addr();
        let remote_port = msg.remote_addr().port();

        let call = msg.associated_call();
        if let Some(call) = call.as_ref() {
            // we only want remote nodes with stable ports in our routing table,
            // so apply a stricter check here
            let mut locked_call = call.lock().unwrap();
            if locked_call.nodeid_mismatched() || locked_call.addr_mismatched() {
                warn!("Received a message from inconsistent node {}@{}, ignored the potential routing table update",
					msg.remote_id(), msg.remote_addr());

                if let Some(detector) = self.suspicious_detector.as_ref() {
                    detector.lock().unwrap().inconsistent(
                        remote_addr.clone(),
                        Some(remote_id.clone())
                    );
                };
                return;
            }

            locked_call.respond(msg);
            if locked_call.is_reachable_at_creation() {
                // TODO: If the call is reachable at creation, it means the remote node is reachable and has a stable port.
                // We can trust the remote address in this case.
                return;
            }
        }

        let last_known_id = self.suspicious_detector.as_ref()
            .and_then(|detector| detector.lock().unwrap().last_known_id(remote_addr).cloned());

        if let Some(known_id) = last_known_id.as_ref() {
            if known_id != remote_id {

                // We already know a node with that address but with a different ID.
                // This might happen if one node changes its ID.
                // Force remove from the routing table to prevent suspicious behavior
                warn!("Received a message from suspicious node {}@{}, force-removing routing table entries because ID-change was detected; new ID {}",
                    msg.remote_id(), msg.remote_addr(), known_id);


                let removed = self.rt.remove(known_id);
                if let Some(_) = removed {
                    // Might be a pollution attack, check other entries in the same bucket too.
                    // In case the random pings can't keep up with scrubbing.
                    let bucket = self.rt.bucket(known_id);
                    info!("Checking bucket {} after ID change was detected", bucket.lock().unwrap().prefix());

                    let prefix = bucket.lock().unwrap().prefix().clone();
                    self.try_ping_maintenance(bucket.clone(), true, false, false,
                        format!("Checking bucket {} after ID change was detected", prefix));
                }

                let removed = self.rt.remove(msg.nodeid());
                if let Some(_) = removed {
                    // Might be a pollution attack, check other entries in the same bucket too.
                    // In case the random pings can't keep up with scrubbing.
                    let bucket = self.rt.bucket(remote_id);
                    // noinspection LoggingSimilarMessage
                    info!("Checking bucket {} after ID change was detected", bucket.lock().unwrap().prefix());

                    let prefix = bucket.lock().unwrap().prefix().clone();
                    self.try_ping_maintenance(bucket, true, false, false,
                        format!("Checking bucket {} after ID change was detected", prefix));
                }

                if let Some(detector) = self.suspicious_detector.as_ref() {
                    detector.lock().unwrap().inconsistent(
                        remote_addr.clone(),
                        Some(remote_id.clone())
                    );
                };
                return;
            }
        }

        let mut existed = false;
        let result = self.rt.bucket_entry(Some(remote_id));
        if let Some(existing) = result {
            if existing.socket_addr() != remote_addr ||
                existing.socket_addr().port() != remote_port {
                warn!("Received a message from inconsistent node {}@{}, ignored the potential routing table update",
					msg.remote_id(), msg.remote_addr());

                if let Some(detector) = self.suspicious_detector.as_ref() {
                    detector.lock().unwrap().inconsistent(
                        remote_addr.clone(),
                        Some(remote_id.clone())
                    );
                };
                return;
            }
            existed = true;
        }

        if let Some(detector) = self.suspicious_detector.as_ref() {
            detector.lock().unwrap().observe(
                remote_addr.clone(),
                remote_id.clone()
            );
        }

        let mut entry = KBucketEntry::new(
            remote_id.clone(),
            remote_addr.clone()
        );
        entry.set_ver(msg.ver());

        if let Some(_call) = call {
            entry.on_responded(0); // TOOD: RTT.
            entry.update_last_sent(_call.lock().unwrap().sent_time());
        }

        self.rt.put(entry.clone());

        // Optimize: not the standard Kademlia behavior
		// incoming request && the new entry is unreachable && the target bucket not full,
		// then try to do a ping request to the new entry check its availability.
        if !existed && !entry.is_reachable(){

            // Verify the node, speed up the bootstrap process or make the bucket more reliable.
			// only if the new entry is unreachable and the bucket is not full yet
            let call = RpcCall::with_entry(
                entry,
                msg::ping_request()
            );
            let _ = self.server().lock().unwrap().send_call(call);
        }
    }

    fn send_err(&mut self, method: Method, code: i32, str: &str) {
        let mut msg = msg::error(method, 0, code, str.into());
        // TODO: set remote id and addr
        let _ = self.server().lock().unwrap().send_msg(&mut msg);
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

        let mut msg = msg::ping_response(txid);
        msg.set_remote(remote_id, remote_addr);

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
    }

    fn fill_closest_nodes(&self, target: Id) -> Vec<NodeInfo> {
        let mut kns = KClosestNodes::new(
            self,
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
        let mut rsp = msg::find_node_response(txid, nodes4, nodes6, token);
        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );

        let _ = self.server().lock().unwrap().send_msg(&mut rsp);
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
            msg::find_value_response_with_nodes(txid, nodes4, nodes6)
        } else {
            msg::find_value_response(txid, value.unwrap())
        };
        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );

        let _ = self.server().lock().unwrap().send_msg(&mut rsp);
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

        _ = self.storage.lock().unwrap().put_value(value.clone(), None);

        let txid = req.txid();
        let mut msg = msg::store_value_response(txid);
        msg.set_remote(
            req.remote_id().clone(),
            remote_addr
        );

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
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
            msg::find_peer_response_with_nodes(txid, nodes4, nodes6)
        } else {
            msg::find_peer_response(txid, peers)
        };

        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );
        let _ = self.server().lock().unwrap().send_msg(&mut rsp);
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
        _ = self.storage.lock().unwrap().put_peer(peer.clone(), None);

        let txid = req.txid();
        let mut msg = msg::announce_peer_response(txid);
        msg.set_remote(
            req.remote_id().clone(),
            remote_addr
        );

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
    }

    pub(crate) fn on_timeout(&mut self, call: &RpcCall) {
        if !self.is_running ||
            !crate::locked!(self.server()).is_reachable() {
             return;
        }

        let nodeid = call.target_id();
        self.rt.on_timeout(&nodeid);
    }

    pub(crate) fn on_send(&mut self, call: &RpcCall) {
        if !self.is_running {
            return;
        }

        let nodeid = call.target_id();
        self.rt.on_request_sent(&nodeid);
    }

    pub(crate) async fn bootstrap(
        dht: Arc<Mutex<Self>>,
        nodes: Vec<NodeInfo>
    ) {
        let mut locked = dht.lock().unwrap();
        if !locked.is_running {
            warn!("Bootstrap failed: DHT is not running.");
            return;
        }
        if nodes.is_empty() {
            warn!("Bootstrap failed: no bootstrap nodes provided.");
            return;
        }

        locked.add_bootstrap_nodes(&nodes);
        if locked.bootstrapping {
            warn!("Bootstrap failed: DHT is already bootstrapping.");
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
        let (network, server) = {
            let mut locked = dht.lock().unwrap();
            if locked.bootstrapping {
                return;
            }
            if crate::elapsed_ms!(locked.last_bootstrap) < Self::BOOTSTRAP_MIN_INTERVAL as u128 {
                return;
            }
            if bootstrap_nodes.is_empty() && locked.rt.is_empty() {
                warn!("no bootstrap nodes provided and routing table is empty.");
                return;
            }

            locked.bootstrapping = true;

            let network = locked.network();
            info!("DHT {}:{} bootstrapping ...", network, locked.id());

            (network, locked.server())
        };

        let mut futures = FuturesUnordered::new();

        for item in bootstrap_nodes.into_iter() {
            let msg = msg::find_node_request(
                Id::random(),
                network.is_ipv4(),
                network.is_ipv6(),
                Some(true)
            );

            let mut call = RpcCall::with_node(item, msg);
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

            let _ = server.lock().unwrap().send_call(call);
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
            (!locked.rt.is_empty(), locked.rt.size())
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

        let node = self.rt.bucket_entry(Some(target)).map(|v| v.into());
        if option == LookupOption::Local {
            return Ok(node);
        }
        if option == LookupOption::Conservative && node.is_some() {
            return Ok(node);
        }

        let promise = Promise::<Option<NodeInfo>>::new();
        let future  = promise.future();

        let mut task = Box::new(NodeLookupTask::new(
            self.weak_dht(),
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
            self.weak_dht(),
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
            self.weak_dht(), value.clone(), expected_seq
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
            self.weak_dht(), valueid, false
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
            self.weak_dht(),
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
            self.weak_dht(), peer.clone(), expected_seq
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
            self.weak_dht(), peer.id().clone(), false
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
