use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime},
    future::Future,
    collections::HashMap,
};
use indexmap::map::IndexMap;
use futures::stream::{FuturesUnordered, StreamExt};
use log::{debug, info, warn, error};

use crate::{
    Id, Network, NodeInfo, PeerInfo, Value, core::version, crypto_identity::CryptoIdentity, dht::task::ping_refresh::PingRefreshTask, errors::Result
};
use crate::dht::{
    utils::{is_any_unicast, is_bogon},
    ConnectionStatus,
    promise::Promise,
    token_manager::TokenManager,
    lookup_option::LookupOption,
    timer,
    consumer::Consumer,
    storage::data_storage::DataStorage,
    rpc::{
        Reachability,
        RpcCall, rpccall::State as CallState,
        rpc_server::RpcServer,
    },
    msg::{
        Message,
        LookupRequest,
        LookupResponse,
        msg::{self, Kind, Method, Body},
    },
    suspicious_node_detector::{
        SuspiciousNodeDetector,
        DefaultSuspiciousNodeDetector
    },
    routing::{
        routing_table::RoutingTable,
        KClosestNodes,
        KBucketEntry,
        KBucket,
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
    }
};

pub(crate) struct DHT {
    identity    : Arc<CryptoIdentity>,
    ni          : Arc<NodeInfo>,

    network     : Network,
    host        : String,
    port        : u16,

    is_running  : bool,
    status      : ConnectionStatus,

    storage     : Arc<Mutex<Box<dyn DataStorage>>>,
    tokenman    : Arc<TokenManager>,
    taskman     : Arc<TaskManager>,

    rt          : Option<Arc<Mutex<RoutingTable>>>,
    server      : Option<Arc<Mutex<RpcServer>>>,

    persist_file    : Option<String>,
    last_saved      : Option<SystemTime>,

    bootstrap_nodes : Vec<NodeInfo>,
    bootstrap_ids   : Vec<Id>,
    last_bootstrap  : SystemTime,
    last_maintenance: SystemTime,
    bootstrapping   : bool,

    timer_client    : Option<Arc<timer::Client>>,

    suspicious_detector: Option<Arc<Mutex<DefaultSuspiciousNodeDetector>>>,

    self_cloned     : Weak<Mutex<DHT>>
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

    pub(crate) fn new_shared(
        identity: Arc<CryptoIdentity>,
        network : Network,
        host    : String,
        port    : u16,
        persist_path    : Option<String>,
        bootstrap_nodes : Vec<NodeInfo>,
        storage : Arc<Mutex<Box<dyn DataStorage>>>,
        tokenman: Arc<TokenManager>
    ) -> Result<Arc<Mutex<Self>>> {

        let nodeid = identity.id().clone();
        let socket_addr = SocketAddr::new(host.parse()?, port);

        Ok(Arc::new_cyclic(|weak_self| {
            Mutex::new(Self {
                identity,
                network,
                host,
                port,
                ni              : Arc::new(NodeInfo::new(nodeid, socket_addr)),
                is_running      : false,
                status          : ConnectionStatus::Disconnected,
                storage,
                tokenman,
                taskman         : Arc::new(TaskManager::new()),
                server          : None,
                rt              : None,
                persist_file    : persist_path,
                last_saved      : None,
                bootstrap_nodes,
                bootstrap_ids   : Vec::new(),
                last_bootstrap  : SystemTime::UNIX_EPOCH,
                last_maintenance: SystemTime::UNIX_EPOCH,
                bootstrapping   : false,
                timer_client    : None,
                suspicious_detector: None,
                self_cloned     : weak_self.clone(),
            })
        }))
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

    pub(crate) fn rt(&self) -> Arc<Mutex<RoutingTable>> {
        self.rt.as_ref()
            .expect("RoutingTable not initialized")
            .clone()
    }

    pub(crate) fn server(&self) -> Arc<Mutex<RpcServer>> {
        self.server.as_ref()
            .expect("RpcServer not initialized")
            .clone()
    }

    fn dht(&self) -> Weak<Mutex<DHT>> {
        self.self_cloned.clone()
    }

    fn timer_client(&self) -> Arc<timer::Client> {
        self.timer_client.as_ref()
            .expect("Timer client not initialized")
            .clone()
    }

    fn fill_home_bucket(dht: Arc<Mutex<DHT>>, nodes: Vec<NodeInfo>) -> impl Future<Output = Result<()>> {
        let promise = Promise::<()>::new();
        let future = promise.future();

        if nodes.is_empty() {
            promise.complete(Ok(()));
            return future;
        }

        let weak_dht = Arc::downgrade(&dht);
        let mut task = Box::new(NodeLookupTask::new(
            weak_dht,
            dht.lock().unwrap().id().clone(),
            false,
        ));
        task.with_name("Bootstrap: filling home bucket".into());
        task.with_bootstrap(true);
        task.with_inject_candidates(nodes);
        task.with_listener(
            TaskListener::new().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        let taskman = dht.lock().unwrap().taskman.clone();
        let _ = taskman.add(task);
        return future;
    }

    fn fill_buckets(dht: Arc<Mutex<DHT>>) -> impl Future<Output = Result<()>> {
        let rt = dht.lock().unwrap().rt();
        let number_of_entries = rt.lock().unwrap().number_of_entries();
        let buckets = rt.lock().unwrap().buckets();

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

            let weak_dht = Arc::downgrade(&dht);
            let mut task = Box::new(NodeLookupTask::new(
                weak_dht,
                lookup_target,
                false,
            ));
            task.with_name(format!("Bootstrap: filling Bucket - {}", bucket_prefix));
            task.with_listener(
                TaskListener::new().ended_fn(
                    move |_| promise.complete(Ok(()))
                )
            );

            let taskman = dht.lock().unwrap().taskman.clone();
            let _ = taskman.add(task);

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
        if self.server().lock().unwrap().is_reachable() {
            return;
        }

        // TODO: if maintenanceTask.

        let refresh_needed = bucket.lock().unwrap().needs_refreshing();
        let replacement_needed = bucket.lock().unwrap().needs_replacement_ping() ||
            bucket.lock().unwrap().is_home_bucket() && bucket.lock().unwrap().find_pingable_replacement().is_some();

        if refresh_needed || replacement_needed /* TODO: maintenance_tasks */ {
            let mut task = Box::new(PingRefreshTask::new(self.dht()));
            task.with_name(name);
            task.with_check_all(check_all);
            task.with_remove_on_timeout(remove_on_timeout);
            // TODO: task.with_probe_replacement(probe_replacement);
            task.with_bucket(bucket);

            //if (maintenanceTasks.putIfAbsent(bucket, task) == null) {
			//	task.addListener(t -> maintenanceTasks.remove(bucket, task));
			//	taskManager.add(task);
			//}

            self.taskman.add(task);
        }
    }

    pub(crate) fn random_lookup(&mut self) {
        if !self.server().lock().unwrap().is_reachable() {
            debug!("Periodic: not performing random lookup, server is uneachable");
            return;
        }

        let mut task = Box::new(NodeLookupTask::new(
            self.dht(),
            Id::random(),
            false,
        ));
        task.with_name(format!("Periodic: random node lookup"));
        let _ = self.taskman.add(task);
    }

    pub(crate) fn random_ping(&mut self) {
        if !self.server().lock().unwrap().is_reachable() {
            info!("Periodic: not performing random ping, server is uneachable");
            return;
        }
        if self.server().lock().unwrap().has_pending_calls() {
            info!("Periodic: not performing random ping, server has pending calls.");
            return;
        }

        let Some(entry) = self.rt().lock().unwrap().bucket_entry(None) else {
            return;
        };

        info!("Periodic: random ping ...");
        let call = RpcCall::with_entry(
            entry,
            msg::ping_request(),
        );

        let _ = self.server().lock().unwrap().send_call(call);
    }

    fn update(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Periodic: DHT update...");
        self.rt_maintenance();

        let entries = self.rt().lock().unwrap().number_of_entries();
        if entries < Self::BOOTSTRAP_IF_LESS_THAN_X_ENTRIES ||
            crate::elapsed_ms!(self.last_bootstrap) > Self::SELF_LOOKUP_INTERVAL {

            let bootstrap_nodes = if entries < Self::USE_BOOTSTRAP_NODES_IF_LESS_THAN_X_ENTRIES {
                self.bootstrap_nodes.clone()
            } else {
                Vec::new()
            };

            let dht = self.dht();
            let _ = tokio::spawn(async move {
                let dht = dht.upgrade()
                    .expect("DHT instance should still be alive");
                Self::do_bootstrap(dht, bootstrap_nodes).await;
            });
        }
    }

    fn rt_maintenance(&mut self) {
        let elapsed = self.last_maintenance
            .elapsed()
            .unwrap_or(Duration::MAX)
            .as_millis();
        if elapsed < Self::ROUTING_TABLE_MAINTENANCE_INTERVAL {
            return;
        }

        info!("Routing table maintenance ...");
        self.last_maintenance = SystemTime::now();

        let dht = self.dht();
        self.rt().lock().unwrap().maintenance(
            self.bootstrap_ids.clone(),
            Consumer::new(move |bucket: Arc<Mutex<KBucket>>| {
                let prefix = bucket.lock().unwrap().prefix().clone();
                let dht = dht.upgrade()
                    .expect("DHT instance should still be alive");
                dht.lock().unwrap().try_ping_maintenance(bucket, false, false, false,
                    format!("Routing table maintenance: refreshing bucket {}", prefix)
                );
            })
        );
    }

    fn persist_routing_table(&mut self) {
        let Some(path) = self.persist_file.as_ref() else {
            return;
        };

        info!("Periodic: persisting routing table ...");
        match self.rt().lock().unwrap().save(path) {
            Ok(()) => self.last_saved = Some(SystemTime::now()),
            Err(err) => error!("Can not save the routing table: {}", err),
        }
    }

    fn purge_suspicious_nodes(&mut self) {
        if let Some(detector) = self.suspicious_detector.as_ref() {
            detector.lock().unwrap().purge();
        }
    }

    fn set_status(&mut self, status: ConnectionStatus) {
        if self.status == status {
            return;
        }

        let old = self.status;
        self.status = status;

        info!("DHT {}:{} connnection status changed: {} -> {}",
            self.network, self.id(), old, self.status
        );

        // TODO:
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        info!("Starting DHT/{}:{} on {} ...", self.network, self.id(), self.addr());

        // initialize routing table
        let mut rt = RoutingTable::new(self.id().clone());
        if let Some(path) = self.persist_file.as_ref() {
            let _ = rt.load(path).map_err(|err| {
                warn!("Failed to load routing table from {}: {}", path, err);
            });
        }

        // initialize RPC server
        let mut server = RpcServer::new(self.ni(), self.identity.clone(), None);
        server.message_handler({
            let dht = self.dht().upgrade()
                .expect("DHT instance should still be alive");
            move |msg| crate::locked!(dht).on_message(msg)
        });
        server.callsent_handler({
            let dht = self.dht().upgrade()
                .expect("DHT instance should still be alive");
            move |call| crate::locked!(dht).on_send(call)
        });
        server.calltimeout_handler({
            let dht = self.dht().upgrade()
                .expect("DHT instance should still be alive");
            move |call| crate::locked!(dht).on_timeout(call)
        });
        server.reachable_handler({
            let dht = self.dht().upgrade()
                .expect("DHT instance should still be alive");
            move |reachable| {
                let mut locked_dht = crate::locked!(dht);
                if reachable {
                    locked_dht.set_status(ConnectionStatus::Connected);
                } else {
                    locked_dht.random_ping();
                    locked_dht.set_status(ConnectionStatus::Disconnected);
                }
            }
        });

        let timer_client = server.timer_client();

        server.start().await?;


       /*  rpc_server::RpcServer::start_reachability_check(server.clone());
        let cloned_server = server.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");
            rt.block_on(rpc_server::run_loop(cloned_server));
        });
        */

        self.rt = Some(Arc::new(Mutex::new(rt)));
        self.server = Some(Arc::new(Mutex::new(server)));
        self.timer_client = Some(timer_client);
        self.set_status(ConnectionStatus::Connecting);

        let startup_dht = self.dht();
        let bootstrap_nodes = self.bootstrap_nodes.clone();
        let network = self.network;
        let nodeid = self.id().clone();

        /*tokio::spawn(async move {
            DHT::do_bootstrap(startup_dht.clone(), bootstrap_nodes).await;

            let mut locked_dht = startup_dht.lock().unwrap();
            info!("DHT {}:{} startup bootstrap finished.", network, nodeid);
            if locked_dht.rt().lock().unwrap().number_of_entries() > 0 {
                locked_dht.set_status(ConnectionStatus::Connected);
            } else {
                locked_dht.set_status(ConnectionStatus::Disconnected);
            }
        });
        */

        self.setup_periodic_tasks()?;
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

        self.taskman.cancel_all();

        if let Some(path) = self.persist_file.take() {
            let rt = self.rt();
            let locked_rt = rt.lock().unwrap();
            let _ = locked_rt.save(path.as_str()).map_err(|err| {
                warn!("Failed to persist routing table to {}: {}", path, err);
            });
               // TODO: self.last_saved = Some(SystemTime::now());
        }

        if let Some(client) = self.timer_client.take() {
            client.stop().await;
        };
        if let Some(server) = self.server.take() {
            let mut locked = server.lock().unwrap();
            locked.reachable_handler(|_| {});
            locked.stop();
        }

        info!("Stopped DHT {}:{} on {}:{}.", self.network, self.id(), self.host, self.port);
    }

    fn setup_periodic_tasks(&mut self) -> Result<()> {
        self.timer_client().add_timer(
            Duration::from_secs(30),
            Some(Duration::from_secs(320)), {
                let dht = self.dht().upgrade()
                    .expect("DHT instance should still be alive");
                move || dht.lock().unwrap().update()
            }
        )?;

        self.timer_client().add_timer(
            Duration::from_millis(Self::RANDOM_LOOKUP_INTERVAL),
            Some(Duration::from_millis(Self::RANDOM_LOOKUP_INTERVAL)), {
                let dht = self.dht().upgrade()
                    .expect("DHT instance should still be alive");
                move || dht.lock().unwrap().random_lookup()
            }
        )?;
        self.timer_client().add_timer(
            Duration::from_millis(Self::RANDOM_PING_INTERVAL),
            Some(Duration::from_millis(Self::RANDOM_PING_INTERVAL)), {
                let dht = self.dht().upgrade()
                    .expect("DHT instance should still be alive");
                move || dht.lock().unwrap().random_ping()
            }
        )?;

        if self.suspicious_detector.is_some() {
            self.timer_client().add_timer(
                Duration::from_secs(60),
                Some(Duration::from_secs(30)), {
                    let dht = self.dht().upgrade()
                        .expect("DHT instance should still be alive");
                    move || dht.lock().unwrap().purge_suspicious_nodes()
                }
            )?;
        }
        if self.persist_file.is_some() {
            self.timer_client().add_timer(
                Duration::from_secs(120),
                Some(Duration::from_millis(Self::ROUTING_TABLE_PERSIST_INTERVAL)), {
                    let dht = self.dht().upgrade()
                        .expect("DHT instance should still be alive");
                    move || dht.lock().unwrap().persist_routing_table()
                }
            )?;
        }
        Ok(())
    }

    fn received(&mut self, locked_msg: &Message) {
        let from_addr  = locked_msg.remote_addr().clone();

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

        let remote_id   = locked_msg.nodeid();
        let remote_addr = locked_msg.remote_addr();
        let remote_port = locked_msg.remote_addr().port();

        let call = locked_msg.associated_call();
        if let Some(call) = call.as_ref() {
            // we only want remote nodes with stable ports in our routing table,
            // so apply a stricter check here
            let locked_call = call.lock().unwrap();
            if locked_call.nodeid_mismatched() || locked_call.addr_mismatched() {
                warn!("Received a message from inconsistent node {}@{}, ignored the potential routing table update",
					locked_msg.remote_id(), locked_msg.remote_addr());

                /*
                self.suspicious_detector.inconsistent(
                    remote_addr.clone(),
                    Some(remote_id.clone())
                );
                */
                return;
            }
        }

        let rt = self.rt();
        let result = self.suspicious_detector.as_mut().map(|_v|
            //v.lock().unwrap().last_known_id(remote_addr).clone()
            Id::random()
        );
        if let Some(known_id) = result.as_ref() {
            if known_id != remote_id {

                // We already know a node with that address but with a different ID.
                // This might happen if one node changes its ID.
                // Force remove from the routing table to prevent suspicious behavior
                warn!("Received a message from suspicious node {}@{}, force-removing routing table entries because ID-change was detected; new ID {}",
                    locked_msg.remote_id(), locked_msg.remote_addr(), known_id);


                let removed = rt.lock().unwrap().remove(known_id);
                if let Some(_) = removed {
                    // Might be a pollution attack, check other entries in the same bucket too.
                    // In case the random pings can't keep up with scrubbing.
                    let bucket = self.rt().lock().unwrap().bucket(known_id);
                    // noinspection LoggingSimilarMessage
                    info!("Checking bucket {} after ID change was detected", bucket.lock().unwrap().prefix());

                    // TODO: try_ping_maintenance(bucket, true, false, false,
                    // "Checking bucket " + bucket.lock().unwrap().prefix() + " after ID change was detected");
                }

                let removed = rt.lock().unwrap().remove(remote_id);
                if let Some(_) = removed {
                    // Might be a pollution attack, check other entries in the same bucket too.
                    // In case the random pings can't keep up with scrubbing.
                    let bucket = rt.lock().unwrap().bucket(remote_id);
                    // noinspection LoggingSimilarMessage
                    info!("Checking bucket {} after ID change was detected", bucket.lock().unwrap().prefix());

                    //TODO: tryPingMaintenance(bucket, true, false, false,
                    // "Checking bucket " + bucket.lock().unwrap().prefix() + " after ID change was detected");
                }

                self.suspicious_detector.as_mut().map(|v|
                    v.lock().unwrap().inconsistent(
                        remote_addr.clone(),
                        Some(remote_id.clone())
                ));
                return;
            }
        }

        let mut existed = false;
        let result = rt.lock().unwrap().bucket_entry(Some(remote_id));
        if let Some(existing) = result {
            if existing.socket_addr() != remote_addr ||
                existing.socket_addr().port() != remote_port {
                warn!("Received a message from inconsistent node {}@{}, ignored the potential routing table update",
					locked_msg.remote_id(), locked_msg.remote_addr());

                self.suspicious_detector.as_mut().map(|v|
                    v.lock().unwrap().inconsistent(
                        remote_addr.clone(),
                        Some(remote_id.clone())
                ));
                return;
            }
            existed = true;
        }

        self.suspicious_detector.as_mut().map(|v|
            v.lock().unwrap().observe(remote_addr.clone(), remote_id.clone())
        );
        let mut entry = KBucketEntry::new(
            remote_id.clone(),
            remote_addr.clone()
        );
        entry.set_ver(locked_msg.ver());

        if let Some(_call) = call {
            entry.on_responded(0); // TOOD: RTT.
            //entry.update_last_sent(call.lock().unwrap().sent_time());
        }

        self.rt().lock().unwrap().put(entry.clone());

        // Optimize: not the standard Kademlia behavior
		// incoming request && the new entry is unreachable && the target bucket not full,
		// then try to do a ping request to the new entry check its availability.
        if !existed && entry.is_reachable(){

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

        let kind = msg.kind();
        match kind {
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
            Method::StoreValue  => self.on_store_value(msg),
            Method::FindPeer    => self.on_find_peer(msg),
            Method::AnnouncePeer=> self.on_announce_peer(msg),
            _                   => self.on_unknown_req(msg),
        }
    }

    fn on_response(&mut self, _: &Message) {}
    fn on_error(&mut self, msg: &Message) {
        let Some(Body::Error(err)) = msg.body() else {
            warn!("Panic: should be error message");
            return;
        };

        warn!("Error from {}/{} - {}:{}, txid {}",
            msg.remote_addr(),
            version::format_version(msg.ver()),
            err.code(),
            err.description(),
            msg.txid()
        );
    }

    fn on_unknown_req(&mut self, msg: &Message) {
        let method = msg.method();

        warn!("Unknown method {} from {}, txid {}",
            method,
            msg.remote_addr(),
            msg.txid()
        );
        self.send_err(method, 203, "unknown method");
    }

    fn on_ping(&mut self, req: &Message) {
        if req.body().is_some() {
            error!("Error: ping request should not have body");
            return;
        }

        let txid = req.txid();
        let remote_id   = req.remote_id().clone();
        let remote_addr = req.remote_addr().clone();

        let mut msg = msg::ping_response(txid);
        msg.set_remote(remote_id, remote_addr);

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
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
            let mut kns = KClosestNodes::new(
                self.rt(), target, KBucket::MAX_ENTRIES,
            );
            kns.fill();
            nodes4 = Some(kns.into())
        }

        if body.want6() && use_ipv6 {
            let mut kns = KClosestNodes::new(
                self.rt(), target, KBucket::MAX_ENTRIES,
            );
            kns.fill();
            nodes6 = Some(kns.into())
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
                let mut kns = KClosestNodes::new(
                    self.rt(), target, KBucket::MAX_ENTRIES,
                );
                kns.fill();
                nodes4 = Some(kns.into());
            }

            if body.want6() && use_ipv6 {
                let mut kns = KClosestNodes::new(
                    self.rt(), target, KBucket::MAX_ENTRIES,
                );
                kns.fill();
                nodes6 = Some(kns.into());
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
                _ = self.send_err(Method::StoreValue, 300, "Cannot replace mismatched mutable/immutable value");
                return;
            }
            if value.sequence_number() < existing.sequence_number() {
                warn!("Rejecting value {}: sequence number {} is less than existing {}", value_id, value.sequence_number(), existing.sequence_number());
                _ = self.send_err(Method::StoreValue, 300, "Sequence number is less than existing value");
                return;
            }
            if body.expected_seq() >= 0 && existing.sequence_number() > body.expected_seq() {
                warn!("Rejecting value {}: existing sequence number {} is greater than expected {}", value_id, existing.sequence_number(), body.expected_seq());
                _ = self.send_err(Method::StoreValue, 300, "Existing sequence number is greater than expected");
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
                let mut kns = KClosestNodes::new(
                    self.rt(), target, KBucket::MAX_ENTRIES,
                );
                kns.fill();
                nodes4 = Some(kns.into());
            }
            if body.want6() && use_ipv6 {
                let mut kns = KClosestNodes::new(
                    self.rt(), target, KBucket::MAX_ENTRIES,
                );
                kns.fill();
                nodes6 = Some(kns.into());
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
            !self.server().lock().unwrap().is_reachable() {
            return;
        }

        let target_id = call.target_id();
        self.rt().lock().unwrap().on_timeout(&target_id);
    }

    pub(crate) fn on_send(&mut self, call: &RpcCall) {
        if !self.is_running {
            return;
        }

        let target_id = call.target_id();
        self.rt().lock().unwrap().on_send(&target_id);
    }

    pub(crate) async fn bootstrap(
        dht: Arc<Mutex<Self>>,
        nodes: Vec<NodeInfo>
    ) {
        {
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
        }
        DHT::do_bootstrap(dht, nodes).await;
    }

    async fn do_bootstrap(
        dht: Arc<Mutex<Self>>,
        mut bootstrap_nodes: Vec<NodeInfo>,
    ) {
        if dht.lock().unwrap().bootstrapping {
            return;
        }
        if crate::elapsed_ms!(dht.lock().unwrap().last_bootstrap) <
            Self::BOOTSTRAP_MIN_INTERVAL as u128 {
            return;
        }

        let rt = dht.lock().unwrap().rt();
        if bootstrap_nodes.is_empty() && rt.lock().unwrap().is_empty() {
            warn!("Bootstrap skipped: no bootstrap nodes provided and routing table is empty.");
            return;
        }

        dht.lock().unwrap().bootstrapping = true;
        info!("DHT {}:{} bootstrapping ...", dht.lock().unwrap().network, dht.lock().unwrap().ni.id());

        let network = dht.lock().unwrap().network();
        let nodeid  = dht.lock().unwrap().id().clone();
        let mut futures = FuturesUnordered::new();

        while let Some(node) = bootstrap_nodes.pop() {
            let msg = msg::find_node_request(
                Id::random(),
                network.is_ipv4(),
                network.is_ipv6(),
                Some(true)
            );

            let mut call = RpcCall::with_node(node, msg);

            let server  = dht.lock().unwrap().server();
            let promise = Promise::<Vec<NodeInfo>>::new();
            let future  = promise.future();

            call.set_state_changed_cb(move |_call, _, cur| {
                if cur.is_final() {
                    let mut nodes = None;

                    if cur == CallState::Responded {
                        let Some(rsp) = _call.rsp() else {
                            promise.complete(Ok([].to_vec()));
                            return;
                        };

                        if let Some(Body::FindNodeResponse(body)) = rsp.body() {
                            nodes = body.nodes(network).map(|v| v.to_vec());
                        };
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

        let rt = dht.lock().unwrap().rt();
        let nodes: Vec<NodeInfo> = nodes.into_values().collect();

        if !nodes.is_empty() && !rt.lock().unwrap().is_empty() {
            _ = DHT::fill_home_bucket(dht.clone(), nodes).await;
        };

        if rt.lock().unwrap().size() > 1 {
            _ = DHT::fill_buckets(dht.clone()).await;
        }

        dht.lock().unwrap().bootstrapping = false;
        dht.lock().unwrap().last_bootstrap = SystemTime::now();

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

        let node = self.rt().lock().unwrap()
            .bucket_entry(Some(target))
            .map(|v| v.into());

        if option == LookupOption::Local {
            return Ok(node);
        }
        if option == LookupOption::Conservative && node.is_some() {
            return Ok(node);
        }

        let promise = Promise::<Option<NodeInfo>>::new();
        let future  = promise.future();

        let mut task = Box::new(NodeLookupTask::new(
            self.dht(),
            target.clone(),
            option != LookupOption::Conservative
        ));
        task.with_name(format!("Lookup node: {}", target));
        task.with_want_target(true);
        task.with_listener(
            TaskListener::new().ended_fn(
                move |t: &mut dyn Task| {
                    let task = t.as_any()
                        .downcast_ref::<NodeLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            })
        );

        let _ = self.taskman.add(task);
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
            self.dht(),
            value_id.clone(),
            expected_seq,
            option != LookupOption::Conservative
        ));
        task.with_name(format!("Lookup value: {}", value_id));
        task.with_listener(
            TaskListener::new().ended_fn(
                move |t: &mut dyn Task| {
                    let task = t.as_any_mut()
                        .downcast_mut::<ValueLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            })
        );

        let _ = self.taskman.add(task);
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

        let mut nested = Box::new(ValueAnnounceTask::new(
            self.dht(), value.clone(), expected_seq
        ));
        nested.with_name(format!("Store value:{}", &value.id()));
        nested.with_listener(
            TaskListener::new().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        // Lookup task to find the closest nodes to the valueid, and
        // then nested announce task to announce the value to those nodes.
        let mut task = Box::new(NodeLookupTask::new(
            self.dht(), value.id(), false
        ));
        task.with_name(format!("Store value: lookup closest node to {}", value.id()));
        task.with_want_token(true);
        task.with_nested(nested);
        task.with_listener({
            TaskListener::new().ended_fn({
                let taskman = taskman.clone();
                move |t: &mut dyn Task| {
                    let task = t.as_any_mut()
                        .downcast_mut::<NodeLookupTask>().unwrap();

                    if task.task_state() != State::Completed {
                        return;
                    }
                    let Some(mut nested) = task.nested_take() else {
                        return;
                    };

                    let closest = task.closest();
                    if closest.is_empty() {
                        // This should never happen
                        warn!("!!! Store value task not started because the node lookup task got the empty closest nodes.");
                        nested.cancel();
                        return;
                    }

                    nested.as_any_mut()
                        .downcast_mut::<ValueAnnounceTask>().unwrap()
                        .with_closest(closest.clone());

                    let _ = taskman.add(nested);
            }})
        });

        let _ = taskman.add(task);
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
            self.dht(),
            peerid.clone(),
            expected_seq,
            expected_count,
            option != LookupOption::Conservative
        ));
        task.with_name(format!("Lookup peer: {}", peerid));
        task.with_listener({
            TaskListener::new().ended_fn(
                move |t: &mut dyn Task| {
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
            self.dht(),
            peer.clone(),
            expected_seq
        ));
        nested.with_name(format!("Announce peer: {}", peer.id()));
        nested.with_listener(
            TaskListener::new().ended_fn(
                move |_| promise.complete(Ok(()))
            )
        );

        // Lookup task to find the closest nodes to the peer, and
        // then nested announce task to announce to those nodes.
        let mut task = Box::new(NodeLookupTask::new(
            self.dht(), peer.id().clone(), false
        ));
        task.with_want_token(true);
        task.with_name(format!("AnnouncePeer: lookup closest node to {}", peer.id()));
        task.with_nested(nested);
        task.with_listener(
            TaskListener::new().ended_fn({
                let taskman = taskman.clone();
                move |t: &mut dyn Task| {
                    let task = t.as_any_mut()
                        .downcast_mut::<NodeLookupTask>().unwrap();

                    if task.task_state() != State::Completed {
                        return;
                    }
                    let Some(mut nested) = task.nested_take() else {
                        return;
                    };

                    let closest = task.closest();
                    if closest.is_empty() {
                        // This should never happen
                        warn!("!!! Peer announce task not started because the node lookup task got the empty closest nodes.");
                        nested.cancel();
                        return;
                    }

                    nested.as_any_mut()
                        .downcast_mut::<PeerAnnounceTask>().unwrap()
                        .with_closest(closest.clone());

                    let _ = taskman.add(nested);
            }})
        );

        let _ =  taskman.add(task);
        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }
}
