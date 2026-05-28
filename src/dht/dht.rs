use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::SystemTime,
    future::Future,
    collections::HashMap,
};
use indexmap::map::IndexMap;
use futures::stream::{FuturesUnordered, StreamExt};
use log::{debug, info, warn, error};

use crate::{
    as_secs,
    Id,
    Network,
    NodeInfo,
    PeerInfo,
    Value,
    core::version,
    crypto_identity::CryptoIdentity,
    errors::Result
};
use crate::dht::{
    utils::{is_any_unicast, is_bogon},
    ConnectionStatus,
   // suspicious_node_detector,

    promise::Promise,
    token_manager::TokenManager,
    lookup_option::LookupOption,
    rpc::{
        rpccall::{self, RpcCall},
        rpc_server::{self, RpcServer},
        rpc_target::Reachability,
    },
    msg::{
        msg::{Kind, Method, Body, Message},
        lookup_req::LookupRequest,
        lookup_rsp::LookupResponse
    },
    storage::data_storage::DataStorage,
    suspicious_node_detector::{
        SuspiciousNodeDetector,
        DefaultSuspiciousNodeDetector
    },
    routing::{
        routing_table::RoutingTable,
        kclosest_nodes::KClosestNodes,
        kbucket_entry::KBucketEntry,
        kbucket::KBucket,
    },
    task::{
        task::{State, Task},
        task_manager::TaskManager,
        task_listener::TaskListener,
        lookup_task::LookupTask,
        node_lookup::NodeLookupTask,
        peer_lookup::PeerLookupTask,
        value_lookup::ValueLookupTask,
        peer_announce::PeerAnnounceTask,
        value_announce::ValueAnnounceTask,
    }
};

pub(crate) struct DHT {
    identity    : Arc<Mutex<CryptoIdentity>>,

    network     : Network,
    host        : String,
    port        : u16,

    ni          : NodeInfo,

    is_running  : bool,
    status      : ConnectionStatus,

    storage     : Arc<Mutex<Box<dyn DataStorage>>>,
    tokenman    : Arc<Mutex<TokenManager>>,

    taskman     : Option<Arc<Mutex<TaskManager>>>,
    rt          : Option<Arc<Mutex<RoutingTable>>>,
    server      : Option<Arc<Mutex<RpcServer>>>,


    persist_file    : Option<String>,
    last_saved      : SystemTime,

    bootstrap_nodes : Vec<NodeInfo>,
    bootstrap_ids   : Vec<Id>,
    last_bootstrap  : SystemTime,
    bootstrapping   : bool,

    suspicious_detector: Option<Arc<Mutex<DefaultSuspiciousNodeDetector>>>,

    self_cloned     : Option<Arc<Mutex<DHT>>>
}

impl DHT {

    const BOOTSTRAP_MIN_INTERVAL: u64 = 4 * 60 * 1000;              // 4 minutes
    const ROUTING_TABLE_PERSIST_INTERVAL: u64 = 10 * 60 * 1000;     // 10 minutes
    const RANDOM_LOOKUP_INTERVAL: u64 = 10 * 60 * 1000;             // 10 minutes
    const RANDOM_PING_INTERVAL  : u64 = 10 * 1000;                  // 10 seconds

    const BOOTSTRAP_IF_LESS_THAN_X_ENTRIES: usize = 30;

    pub(crate) fn new(
        identity: Arc<Mutex<CryptoIdentity>>,
        network : Network,
        host    : String,
        port    : u16,
        persist_path    : Option<String>,
        bootstrap_nodes : Vec<NodeInfo>,
        storage : Arc<Mutex<Box<dyn DataStorage>>>,
        tokenman: Arc<Mutex<TokenManager>>
    ) -> Result<Self> {

        let nodeid = identity.lock().unwrap().id().clone();
        let socket_addr = SocketAddr::new(host.parse()?, port);

        Ok(Self {
            identity,
            network,
            host,
            port,
            ni              : NodeInfo::new(nodeid, socket_addr),
            is_running      : false,
            status          : ConnectionStatus::Disconnected,
            storage,
            tokenman,
            taskman         : None,
            server          : None,
            rt              : None,
            persist_file    : persist_path,
            last_saved      : SystemTime::UNIX_EPOCH,
            bootstrap_nodes,
            bootstrap_ids   : Vec::new(),
            last_bootstrap  : SystemTime::UNIX_EPOCH,
            bootstrapping   : false,
            suspicious_detector: None,
            self_cloned     : None,
        })
    }

    pub(crate) fn network(&self) -> Network {
        self.network
    }

    pub(crate) fn ni(&self) -> &NodeInfo {
        &self.ni
    }

    pub(crate) fn id(&self) -> &Id {
        self.ni.id()
    }

    pub(crate) fn addr(&self) -> &SocketAddr {
        self.ni.socket_addr()
    }

    pub(crate) fn set_cloned(&mut self, cloned: Arc<Mutex<DHT>>) {
        self.self_cloned = Some(cloned);
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

    fn taskman(&self) -> Arc<Mutex<TaskManager>> {
        self.taskman.as_ref()
            .expect("TaskManager not initialized")
            .clone()
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.self_cloned.as_ref()
            .expect("DHT is not set yet")
            .clone()
    }

    fn fill_home_bucket(dht: Arc<Mutex<DHT>>, nodes: Vec<NodeInfo>) -> impl Future<Output = Result<()>> {
        let promise = Promise::<()>::new();
        let future = promise.future();

        if nodes.is_empty() {
            promise.complete(Ok(()));
            return future;
        }

        let mut task = Box::new(NodeLookupTask::new(
            dht.clone(),
            dht.lock().unwrap().id().clone(),
            false,
        ));
        task.with_name("Bootstrap: filling home bucket".into());
        task.with_bootstrap(true);
        task.with_inject_candidates(nodes);
        task.with_listener(
            TaskListener::new().ended_fn(
                Box::new(move |_| promise.complete(Ok(())))
            )
        );

        let task = Arc::new(Mutex::new(task as Box<dyn Task>));
        let taskman = dht.lock().unwrap().taskman().clone();
        let _ = taskman.lock().unwrap().add(task);
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

            let mut task = Box::new(NodeLookupTask::new(
                dht.clone(),
                lookup_target,
                false,
            ));
            task.with_name(format!("Bootstrap: filling Bucket - {}", bucket_prefix));
            task.with_listener(
                TaskListener::new().ended_fn(
                    Box::new(move |_| promise.complete(Ok(())))
                )
            );

            let task = Arc::new(Mutex::new(task as Box<dyn Task>));
            //task.lock().unwrap().set_cloned(task.clone());

            let taskman = dht.lock().unwrap().taskman().clone();
            let _ = taskman.lock().unwrap().add(task);

            futures_unordered.push(future);
        }

        return async move {
            while let Some(result) = futures_unordered.next().await {
                result?;
            }
            Ok(())
        }
    }

    pub(crate) fn random_lookup(&mut self) {
        if !self.server().lock().unwrap().is_reachable() {
            debug!("Periodic: not performing random lookup, server is uneachable");
            return;
        }

        let task = Arc::new(Mutex::new({
            let mut task = Box::new(NodeLookupTask::new(
                self.dht(),
                Id::random(),
                false,
            ));
            task.with_name(format!("Periodic: random node lookup"));
            task as Box<dyn Task>
        }));

        //task.lock().unwrap().set_cloned(task.clone());
        let _ = self.taskman().lock().unwrap().add(task);
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

        let Some(entry) = self.rt().lock().unwrap().random_kentry() else {
            return;
        };

        info!("Periodic: random ping ...");
        let msg = Message::ping_req();
        let call = Arc::new(Mutex::new(RpcCall::with_kentry(
            entry,
            msg
        )));
        call.lock().unwrap().set_cloned(call.clone());
        let _ = self.server().lock().unwrap().send_call(call);
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

        let rt = Arc::new(Mutex::new(RoutingTable::new(self.id().clone())));
        let taskman = Arc::new(Mutex::new(TaskManager::new()));

        let server  = Arc::new(Mutex::new({

            let addr = self.ni.socket_addr().clone();
            let identity = self.identity.clone();

            let mut server = RpcServer::new(addr, identity, None);

            let dht = self.dht();
            server.set_message_cb({
                let dht = dht.clone();
                move |msg| dht.lock().unwrap().on_message(msg)
            });
            server.set_call_sent_cb({
                let dht = dht.clone();
                move |call| dht.lock().unwrap().on_send(call)
            });
            server.set_call_timeout_cb({
                let dht = dht.clone();
                move |call| dht.lock().unwrap().on_timeout(call)
            });
            server.set_reachable_cb({
                let dht = dht.clone();
                move |reachable| {
                    let mut locked_dht = dht.lock().unwrap();
                    if reachable {
                        locked_dht.set_status(ConnectionStatus::Connected);
                    } else {
                        locked_dht.random_ping();
                        locked_dht.set_status(ConnectionStatus::Disconnected);
                    }
                }
            });
            server.start()?;
            server
        }));

        let cloned_server = server.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");
            rt.block_on(rpc_server::run_loop(cloned_server));
        });

        self.taskman = Some(taskman);
        self.server = Some(server);
        self.rt = Some(rt);

        self.set_status(ConnectionStatus::Connecting);

        let startup_dht = self.dht();
        let bootstrap_nodes = self.bootstrap_nodes.clone();
        let network = self.network;
        let nodeid = self.id().clone();

        tokio::spawn(async move {
            DHT::do_bootstrap(startup_dht.clone(), bootstrap_nodes).await;

            let mut locked_dht = startup_dht.lock().unwrap();
            info!("DHT {}:{} startup bootstrap finished.", network, nodeid);
            if locked_dht.rt().lock().unwrap().number_of_entries() > 0 {
                locked_dht.set_status(ConnectionStatus::Connected);
            } else {
                locked_dht.set_status(ConnectionStatus::Disconnected);
            }
        });

        self.setup_periodic_tasks();
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
        self.set_status(ConnectionStatus::Disconnected);

        self.bootstrapping = false;

        if let Some(taskman) = self.taskman.take() {
            taskman.lock().unwrap().cancel_all();
        }

        {
            let server = self.server();
            let mut locked = server.lock().unwrap();
            locked.set_reachable_cb(|_| {});
            locked.stop();
        }

        info!("Stopped DHT {}:{} on {}:{}.", self.network, self.id(), self.host, self.port);
    }

    fn setup_periodic_tasks(&mut self) {
        //unimplemented!()
        /*
        let scheduler = self.scheduler.lock().unwrap();

        // Regular dht update.
        let cloned_dht = self.dht();
        scheduler.lock().unwrap().add(move || {
            //cloned_dht.lock().unwrap().update();
            unimplemented!()
        }, 30*1000, 30*1000);

        // check socket liveness.
        let cloned_dht = self.dht();
        scheduler.lock().unwrap().add(move || {
            cloned_dht.lock().unwrap().random_ping();
        }, Self::RANDOM_PING_INTERVAL, Self::RANDOM_PING_INTERVAL);

        // Perform a deep lookup to familiarize ourselves with random sections of
        // the keyspace.
        let cloned_dht = self.dht();
        scheduler.lock().unwrap().add(move || {
            cloned_dht.lock().unwrap().random_lookup();
        }, Self::RANDOM_LOOKUP_INTERVAL, Self::RANDOM_LOOKUP_INTERVAL);

        // TODO: handle suspicious nodes.

        if let Some(path) = self.store_path.as_ref() {
            let cloned_rt  = self.rt();
            let path = path.to_string();
            scheduler.lock().unwrap().add(move || {
                info!("Persisting routing table ....");
                cloned_rt.lock().unwrap().save(&path);
            }, 120*1000, Self::ROUTING_TABLE_PERSIST_INTERVAL);
        }
        */
    }

    fn received(&mut self, locked_msg: &mut Message) {
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

        let remote_id   = locked_msg.id();
        let remote_addr = locked_msg.remote_addr();
        let remote_port = locked_msg.remote_addr().port();

        let call = locked_msg.associated_call();
        if let Some(call) = call.as_ref() {
            // we only want remote nodes with stable ports in our routing table,
            // so apply a stricter check here
            let locked_call = call.lock().unwrap();
            if locked_call.id_mismatched() || locked_call.addr_mismatched() {
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
        let result = rt.lock().unwrap().bucket_entry(remote_id);
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
            let msg = Message::ping_req();
            let call = Arc::new(Mutex::new(RpcCall::with_kentry(
                entry,
                msg
            )));

            call.lock().unwrap().set_cloned(call.clone());
            let _ = self.server().lock().unwrap().send_call(call);
        }
    }

    fn send_err(&mut self, method: Method, code: i32, str: &str) {
        let mut msg = Message::error(method, 0, code, str.into());
        // TODO: set remote id and addr
        let _ = self.server().lock().unwrap().send_msg(&mut msg);
    }

    pub(crate) fn on_message(&mut self, msg: &mut Message) {
        if !self.is_running {
            return;
        }
        // ignore the messages from myself
        if self.id() == msg.id() {
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
            err.msg(),
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

        let mut msg = Message::ping_rsp(txid);
        msg.set_remote(remote_id, remote_addr);

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
    }

    fn on_find_node(&mut self, req: &Message) {
        let Some(Body::FindNodeReq(body)) = req.body() else {
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
            token = self.tokenman.lock().unwrap().generate_token(
                req.id(),
                req.remote_addr(),
                &target
            );
        }

        let txid = req.txid();
        let mut rsp = Message::find_node_rsp(txid, nodes4, nodes6, token);
        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );

        let _ = self.server().lock().unwrap().send_msg(&mut rsp);
    }

    fn on_find_value(&mut self, req: &Message) {
        let Some(Body::FindValueReq(body)) = req.body() else {
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
            Message::find_value_rsp_with_nodes(txid, nodes4, nodes6)
        } else {
            Message::find_value_rsp(txid, value.unwrap())
        };
        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );

        let _ = self.server().lock().unwrap().send_msg(&mut rsp);
    }

    fn on_store_value(&mut self, req: &Message) {
        let Some(Body::StoreValueReq(body)) = req.body() else {
            error!("Error: should be store value request");
            return;
        };

        let value = body.value();
        let value_id = value.id();
        let remote_addr = req.remote_addr().clone();

        let is_valid = self.tokenman.lock().unwrap().verify_token(
            body.token(),
            req.id(),
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
        let mut msg = Message::store_value_rsp(txid);
        msg.set_remote(
            req.remote_id().clone(),
            remote_addr
        );

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
    }

    fn on_find_peer(&mut self, req: &Message) {
        let Some(Body::FindPeerReq(body)) = req.body() else {
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
            Message::find_peer_rsp_with_nodes(txid, nodes4, nodes6)
        } else {
            Message::find_peer_rsp(txid, peers)
        };

        rsp.set_remote(
            req.remote_id().clone(),
            req.remote_addr().clone()
        );
        let _ = self.server().lock().unwrap().send_msg(&mut rsp);
    }

    fn on_announce_peer(&mut self, req: &Message) {
        let Some(Body::AnnouncePeerReq(body)) = req.body() else {
            error!("Panic: should be announce peer request");
            return;
        };

        let peer = body.peer();
        let remote_addr = req.remote_addr().clone();
        let is_valid = self.tokenman.lock().unwrap().verify_token(
            body.token(),
            req.id(),
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
        let mut msg = Message::announce_peer_rsp(txid);
        msg.set_remote(
            req.remote_id().clone(),
            remote_addr
        );

        let _ = self.server().lock().unwrap().send_msg(&mut msg);
    }

    pub(crate) fn on_timeout(&mut self, call: Arc<Mutex<RpcCall>>) {
        if !self.is_running ||
            !self.server().lock().unwrap().is_reachable() {
            return;
        }

        let target_id = call.lock().unwrap().target_id();
        self.rt().lock().unwrap().on_timeout(&target_id);
    }

    pub(crate) fn on_send(&mut self, call: Arc<Mutex<RpcCall>>) {
        if !self.is_running {
            return;
        }

        let target_id = call.lock().unwrap().target_id();
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
        if as_secs!(dht.lock().unwrap().last_bootstrap) < Self::BOOTSTRAP_MIN_INTERVAL {
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
            let msg = Message::find_node_req(
                Id::random(),
                network.is_ipv4(),
                network.is_ipv6(),
                true
            );

            let call = Arc::new(Mutex::new(RpcCall::with_node(
                node,
                msg
            )));


            let server  = dht.lock().unwrap().server();
            let promise = Promise::<Vec<NodeInfo>>::new();
            let future  = promise.future();

            call.lock().unwrap().set_cloned(call.clone());
            call.lock().unwrap().set_state_changed_cb(move |_call, _, cur| {
                if cur.is_final() {
                    let mut nodes = None;

                    if cur == rpccall::State::Responded {
                        let Some(rsp) = _call.rsp() else {
                            promise.complete(Ok([].to_vec()));
                            return;
                        };

                        if let Some(Body::FindNodeRsp(body)) = rsp.body() {
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
        dht: Arc<Mutex<Self>>,
        target: &Id,
        option: LookupOption
    ) -> Result<Option<NodeInfo>> {

        let rt  = dht.lock().unwrap().rt();
        let node = rt.lock().unwrap().bucket_entry(target).map(|v| v.into());
        let taskman = dht.lock().unwrap().taskman();

        if option == LookupOption::Local {
            return Ok(node);
        }
        if option == LookupOption::Conservative && node.is_some() {
            return Ok(node);
        }

        let promise = Promise::<Option<NodeInfo>>::new();
        let future  = promise.future();

        let mut task = NodeLookupTask::new(
            dht.clone(),
            target.clone(),
            option != LookupOption::Conservative
        );
        task.with_name(format!("Lookup node: {}", target));
        task.with_want_target(true);
        task.with_listener(
            TaskListener::new().ended_fn(
                Box::new(move |t: &dyn Task| {
                    let task = t.as_any().downcast_ref::<NodeLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            }))
        );

        let _ = taskman.lock().unwrap().add(
            Arc::new(Mutex::new(Box::new(task)))
        );

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn find_value(
        dht: Arc<Mutex<Self>>,
        value_id: &Id,
        expected_seq: i32,
        option: LookupOption
    ) -> Result<Option<Value>> {

        let taskman = dht.lock().unwrap().taskman();
        let promise = Promise::<Option<Value>>::new();
        let future  = promise.future();

        let mut task = ValueLookupTask::new(
            dht.clone(),
            value_id.clone(),
            expected_seq,
            option != LookupOption::Conservative
        );
        task.with_name(format!("Lookup value: {}", value_id));
        task.with_listener(
            TaskListener::new().ended_fn(
                Box::new(move |t: &dyn Task| {
                    let task = t.as_any().downcast_ref::<ValueLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            }))

        );

        let _ = taskman.lock().unwrap().add(
            Arc::new(Mutex::new(Box::new(task)))
        );

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn store_value(
        dht: Arc<Mutex<Self>>,
        value: Value,
        expected_seq: i32
    ) -> Result<()> {

        let taskman = dht.lock().unwrap().taskman();
        let promise = Promise::<()>::new();
        let future  = promise.future();

        let announce_task = {
            let mut task = ValueAnnounceTask::new(
                dht.clone(), value.clone(), expected_seq
            );
            task.with_name(format!("Store value:{}", &value.id()));
            task.with_listener(
                TaskListener::new().ended_fn(
                    Box::new(move |_| promise.complete(Ok(())))
                )
            );
            Arc::new(Mutex::new(
                Box::new(task) as Box<dyn Task>)
            )
        };

        let mut task = NodeLookupTask::new(
            dht.clone(),value.id(), false
        );
        task.with_name(format!("Store value: lookup closest node to {}", value.id()));
        task.with_want_token(true);
        task.set_nested(announce_task.clone());
        task.with_listener({
            TaskListener::new().ended_fn(
                Box::new(move |t: &dyn Task| {
                    let task = t.as_any().downcast_ref::<NodeLookupTask>().unwrap();
                    if task.state() != State::Completed {
                        return;
                    }

                    let announce_task = announce_task.clone();
                    let mut locked = announce_task.lock().unwrap();
                    let closest = task.closest();
                    if closest.is_empty() {
                        // This should never happen
                        warn!("!!! Store value task not started because the node lookup task got the empty closest nodes.");
                        locked.cancel();
                        return;
                    }

                    let task = locked.as_any_mut().downcast_mut::<ValueAnnounceTask>().unwrap();
                    task.with_closest(closest.clone());
                    drop(locked);

                    let _ = taskman.lock().unwrap().add(announce_task);
            }))
        });

        let taskman = dht.lock().unwrap().taskman();
        let _ = taskman.lock().unwrap().add(
            Arc::new(Mutex::new(Box::new(task) as Box<dyn Task>))
        );

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn find_peer(
        dht: Arc<Mutex<Self>>,
        peerid: &Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption
    ) -> Result<Vec<PeerInfo>> {

        let taskman = dht.lock().unwrap().taskman();
        let promise = Promise::<Vec<PeerInfo>>::new();
        let future  = promise.future();

        let mut task = PeerLookupTask::new(
            dht.clone(),
            peerid.clone(),
            expected_seq,
            expected_count,
            option != LookupOption::Conservative
        );
        task.with_name(format!("Lookup peer: {}", peerid));
        task.with_listener({
            TaskListener::new().ended_fn(
                Box::new(move |t: &dyn Task| {
                    let task = t.as_any().downcast_ref::<PeerLookupTask>().unwrap();
                    promise.complete(Ok(task.result()));
            }))
        });

        let _ = taskman.lock().unwrap().add(
            Arc::new(Mutex::new(Box::new(task)))
        );

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }

    pub(crate) async fn announce_peer(
        dht: Arc<Mutex<Self>>,
        peer: PeerInfo,
        expected_seq: i32
    ) -> Result<()>{
        let taskman = dht.lock().unwrap().taskman();
        let promise = Promise::<()>::new();
        let future  = promise.future();

        let announce_task = {
            let mut task = PeerAnnounceTask::new(
                dht.clone(),
                peer.clone(),
                expected_seq
            );
            task.with_name(format!("Announce peer: {}", peer.id()));
            task.with_listener(
                TaskListener::new().ended_fn(
                    Box::new(move |_| promise.complete(Ok(())))
                )
            );

            Arc::new(Mutex::new(
                Box::new(task) as Box<dyn Task>
            ))
        };
        let mut task = NodeLookupTask::new(
            dht.clone(), peer.id().clone(), false
        );
        task.with_want_token(true);
        task.with_name(format!("AnnouncePeer: lookup closest node to {}", peer.id()));
        task.set_nested(announce_task.clone());
        task.with_listener(
            TaskListener::new().ended_fn(
                Box::new(move |t: &dyn Task| {
                    let task = t.as_any().downcast_ref::<NodeLookupTask>().unwrap();
                    if t.state() != State::Completed {
                        return;
                    }

                    let announce_task = announce_task.clone();
                    let mut locked = announce_task.lock().unwrap();
                    let closest = task.closest();
                    if closest.is_empty() {
                        // This should never happen
                        warn!("!!! Peer announce task not started because the node lookup task got the empty closest nodes.");
                        locked.cancel();
                        return;
                    }

                    let task = locked.as_any_mut().downcast_mut::<PeerAnnounceTask>().unwrap();
                    task.with_closest(closest.clone());
                    drop(locked);

                    let _ = taskman.lock().unwrap().add(announce_task);
            }))
        );

        let taskman = dht.lock().unwrap().taskman();
        let _ =  taskman.lock().unwrap().add(
            Arc::new(Mutex::new(Box::new(task) as Box<dyn Task>))
        );

        match future.clone().await {
            Ok(_) => future.result(),
            Err(e) => Err(e)
        }
    }
}
