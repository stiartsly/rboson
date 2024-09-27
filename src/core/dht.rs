
use std::rc::Rc;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;
use std::ops::Deref;
use std::hash::{DefaultHasher, Hash, Hasher};
use log::{debug, info, warn, error, trace};

use crate::{
    unwrap,
    is_bogon_addr,
    addr_family,
    as_millis,
};

use crate::{
    Id,
    Network,
    NodeInfo,
    PeerInfo,
    Value,
    Error
};

use crate::core::{
    constants,
    version,
    rpccall,
    rpccall::RpcCall,
    server::Server,
    token_manager::TokenManager,
    data_storage::DataStorage,
    lookup_option::LookupOption,
    routing_table::RoutingTable,
    kclosest_nodes::KClosestNodes,
    kbucket_entry::KBucketEntry,
};

use crate::core::msg::{
    lookup_req::Msg as LookupRequest,
    lookup_rsp::Msg as LookupResponse,
    msg::{Msg, Kind, Method},
};

use crate::core::task::{
    task::{State, Task},
    lookup_task::LookupTask,
    node_lookup::NodeLookupTask,
    peer_lookup::PeerLookupTask,
    task_manager::TaskManager,
    value_lookup::ValueLookupTask,
    value_announce::ValueAnnounceTask,
    peer_announce::PeerAnnounceTask,
    ping_refresh::PingOption,
};

pub(crate) struct DHT {
    id: Rc<Id>,
    ni: Rc<NodeInfo>,
    store_path: Option<String>,
    last_saved: SystemTime,
    running: bool,

    bootstrap_needed: bool,
    bootstrap_nodes : Vec<Rc<NodeInfo>>,
    bootstrap_time  : Rc<RefCell<SystemTime>>,

    known_nodes: HashMap<SocketAddr, Id>,

    rt:     Rc<RefCell<RoutingTable>>,
    taskman:Rc<RefCell<TaskManager>>,

    server: Option<Rc<RefCell<Server>>>,
    tokman: Option<Rc<RefCell<TokenManager>>>,
    storage:Option<Rc<RefCell<dyn DataStorage>>>,
    cloned: Option<Rc<RefCell<DHT>>>,
}

impl DHT {
    pub(crate) fn new(nodeid: Rc<Id>, binding_addr: SocketAddr) -> Self {
        DHT {
            id: Rc::clone(&nodeid),
            ni: Rc::new(NodeInfo::new((*nodeid).clone(), binding_addr)),
            running: false,
            store_path: None,
            last_saved: SystemTime::UNIX_EPOCH,

            bootstrap_nodes: Vec::new(),
            bootstrap_needed: false,
            bootstrap_time: Rc::new(RefCell::new(SystemTime::UNIX_EPOCH)),

            known_nodes: HashMap::new(),

            rt:     Rc::new(RefCell::new(RoutingTable::new(nodeid.clone()))),
            taskman:Rc::new(RefCell::new(TaskManager::new())),

            server: None,
            storage:None,
            tokman: None,
            cloned: None,
        }
    }

    pub(crate) fn set_field<T: 'static>(&mut self, field: T) -> &mut Self {
        let type_id = TypeId::of::<T>();
        let field_any = Box::new(field) as Box<dyn Any>;

        if type_id == TypeId::of::<Rc<RefCell<DHT>>>() {
            self.cloned = Some(field_any.downcast::<Rc<RefCell<DHT>>>().unwrap().deref().clone());
        } else if type_id == TypeId::of::<Rc<RefCell<Server>>>() {
            self.server = Some(field_any.downcast::<Rc<RefCell<Server>>>().unwrap().deref().clone());
        } else if type_id == TypeId::of::<Rc<RefCell<dyn DataStorage>>>() {
            self.storage = Some(field_any.downcast::<Rc<RefCell<dyn DataStorage>>>().unwrap().deref().clone());
        } else if type_id == TypeId::of::<Rc<RefCell<TokenManager>>>() {
            self.tokman = Some(field_any.downcast::<Rc<RefCell<TokenManager>>>().unwrap().deref().clone());
        }
        self
    }

    pub(crate) fn enable_persistence(&mut self, path: String) {
        self.store_path = Some(path);
    }

    pub(crate) fn addr(&self) -> &SocketAddr {
        self.ni.socket_addr()
    }

    pub(crate) fn id(&self) -> &Id {
        self.ni.id()
    }

    pub(crate) fn rc_id(&self) -> Rc<Id> {
        self.id.clone()
    }

    pub(crate) fn network(&self) -> Network {
        Network::from(self.addr())
    }

    pub(crate) fn ni(&self) -> Rc<NodeInfo> {
        self.ni.clone()
    }

    pub(crate) fn rt(&self) -> Rc<RefCell<RoutingTable>> {
        self.rt.clone()
    }

    pub(crate) fn taskman(&self) -> Rc<RefCell<TaskManager>> {
        self.taskman.clone()
    }

    pub(crate) fn server(&self) -> Rc<RefCell<Server>> {
        unwrap!(self.server).clone()
    }

    fn dht(&self) -> Rc<RefCell<DHT>> {
        unwrap!(self.cloned).clone()
    }

    fn storage(&self) -> &Rc<RefCell<dyn DataStorage>> {
        unwrap!(self.storage)
    }

    pub(crate) fn add_bootstrap_node(&mut self, node: Rc<NodeInfo>) {
        self.bootstrap_nodes.push(node)
    }

    pub(crate) fn bootstrap(&mut self) {
        let mut bootstr_nodes = self.bootstrap_nodes.clone();
        if  bootstr_nodes.is_empty() {
            bootstr_nodes = self.rt.borrow()
                .random_entries(8)
                .iter()
                .map(|item| item.borrow().ni())
                .collect();
        }

        debug!("DHT/{} bootstraping ....", addr_family!(self.addr()));

        let bootstr_map = Rc::new(RefCell::new(HashMap::new()));
        let count = Rc::new(RefCell::new(0));

        for item in bootstr_nodes.iter() {
            let req = Rc::new(RefCell::new({
                use crate::core::msg::find_node_req as req;
                let mut msg = Box::new(req::Message::new());

                msg.set_remote(item.id(), item.socket_addr());
                msg.with_target(Rc::new(Id::random()));
                msg.with_want4(true);
                msg as Box<dyn Msg>
            }));

            let call = Rc::new(RefCell::new(RpcCall::new(
                item.clone(),
                self.dht(),
                req
            )));

            let bootstr_sz = bootstr_nodes.len();
            let cloned_map = bootstr_map.clone();
            let cloned_cnt = count.clone();
            let cloned_dht = self.dht();
            let cloned_bootstrap_time = self.bootstrap_time.clone();

            call.borrow_mut().set_cloned(call.clone());
            call.borrow_mut().set_state_changed_fn(move |_call, _, _cur| {
                match _cur {
                    rpccall::State::Responsed => {},
                    rpccall::State::Err => {},
                    rpccall::State::Timeout => {},
                    _ => return,
                }

                // Process the closest nodes found in the response message.
                _call.rsp().map(|msg| {
                    use crate::core::msg::find_node_rsp as rsp;
                    msg.borrow().as_any().downcast_ref::<rsp::Message>().map(|downcasted| {
                        downcasted.nodes4().map(|nodes| {
                            nodes.iter().for_each(|ni| {
                                cloned_map.borrow_mut().insert(
                                    ni.id().clone(),
                                    ni.clone()
                                );
                            })
                        });
                        downcasted.nodes6().map(|nodes| {
                            nodes.iter().for_each(|ni| {
                                cloned_map.borrow_mut().insert(
                                    ni.id().clone(),
                                    ni.clone()
                                );
                            })
                        });
                    });
                });

                *cloned_cnt.borrow_mut() += 1;
                if *cloned_cnt.borrow() == bootstr_sz {
                    *cloned_bootstrap_time.borrow_mut() = SystemTime::now();
                    cloned_dht.borrow().fill_home_bucket(
                        cloned_map.borrow().values().cloned().collect()
                    );
                }
            });

            self.server().borrow_mut().send_call(call);
        };
    }

    fn fill_home_bucket(&self, nodes: Vec<Rc<NodeInfo>>) {
        if self.rt.borrow().size() == 0 && nodes.is_empty() {
            return;
        }

        let task = Rc::new(RefCell::new({
            let mut task = Box::new(NodeLookupTask::new(
                self.rc_id(),
                self.dht()
            ));
            task.set_bootstrap(true);
            task.inject_candidates(&nodes);
            task.set_name("NodeLookup: Filling home bucket");
            task.add_listener(Box::new(move |_| {
                // TODO:
            }));
            task as Box<dyn Task>
        }));
        task.borrow_mut().set_cloned(task.clone());
        self.taskman.borrow_mut().add(task);
    }

    pub(crate) fn update(&mut self) {
        if !self.running {
            return;
        }

        trace!("DHT/{} regularly update...", addr_family!(self.addr()));

        self.server().borrow_mut().update_reachability();
        self.rt.borrow_mut().maintenance();

        if self.bootstrap_needed ||
            self.rt.borrow().size_of_entries() < constants::BOOTSTRAP_IF_LESS_THAN_X_PEERS ||
            as_millis!(self.bootstrap_time.borrow()) > constants::SELF_LOOKUP_INTERVAL {

            // Regularly search for our ID to update the routing table
            self.bootstrap_needed = false;
            self.bootstrap();
        }

        if as_millis!(self.last_saved) > constants::ROUTING_TABLE_PERSIST_INTERVAL as u128 {
            info!("Persisting routing table ....");
            self.rt.borrow_mut().save(self.store_path.as_ref().unwrap().as_str());
            self.last_saved = SystemTime::now();
        }
    }

    pub(crate) fn random_ping(&mut self) {
        if self.server().borrow().number_of_acitve_calls() > 0 {
            return;
        }

        let ni = match self.rt.borrow().random_entry() {
            Some(v) => v.borrow().ni(),
            None => return,
        };

        let call = Rc::new(RefCell::new({
            use crate::core::msg::ping_req as req;
            RpcCall::new(ni, self.dht(), Rc::new(RefCell::new(
                Box::new(req::Message::new()) as Box<dyn Msg>
            )))
        }));

        call.borrow_mut().set_cloned(call.clone());
        self.server().borrow_mut().send_call(call);
    }

    pub(crate) fn random_lookup(&mut self) {
        let task = Rc::new(RefCell::new({
            let mut task_ = Box::new(NodeLookupTask::new(
                Rc::new(Id::random()),
                self.dht()
            ));
            task_.set_name("NodeLookup: Random refresh");
            task_.add_listener(Box::new(move |_|{}));
            task_ as Box<dyn Task>
        }));

        task.borrow_mut().set_cloned(task.clone());
        self.taskman.borrow_mut().add(task);
    }

    pub(crate) fn start(&mut self) -> Result<SocketAddr, Error> {
        if self.running {
            return Err(Error::State(format!("DHT node is already running")));
        }

        // Load neighboring nodes from cache storage if they exist.
        if let Some(path) = self.store_path.as_ref() {
            info!("Loading routing table from [{}] ...", path);
            self.rt.borrow_mut().load(path);
        }

        info!("Starting DHT/{} on {}", addr_family!(self.addr()), self.addr());
        self.running  = true;
        let scheduler = self.server().borrow().scheduler();
        let taskman   = self.taskman.clone();
        scheduler.borrow_mut().add(move || {
            taskman.borrow_mut().dequeue();
        }, 500, constants::DHT_UPDATE_INTERVAL);

        // fix the first time to persist the routing table: 2 min
        // TODO: self.last_saved = SystemTime::now() - Duration::from_millis(constants::ROUTING_TABLE_PERSIST_INTERVAL + (120 * 1000));

        self.rt().borrow_mut().set_dht(self.dht());
        // Regular dht update.
        let dht = self.dht();
        scheduler.borrow_mut().add(move || {
            dht.borrow_mut().update();
        }, 100, constants::DHT_UPDATE_INTERVAL);

        // Send a ping request to a random node to verify socket liveness.
        let dht = self.dht();
        scheduler.borrow_mut().add(move || {
            dht.borrow_mut().random_ping();
        }, constants::RANDOM_PING_INTERVAL, constants::RANDOM_PING_INTERVAL);

        // Perform a deep lookup to familiarize ourselves with random sections of
        // the keyspace.
        let dht = self.dht();
        scheduler.borrow_mut().add(move || {
            dht.borrow_mut().random_lookup();
        }, constants::RANDOM_LOOKUP_INTERVAL, constants::RANDOM_LOOKUP_INTERVAL);

        Ok(self.addr().clone())
    }

    pub(crate) fn stop(&mut self) {
        if !self.running {
            return;
        }

        info!("{} started on shutdown ...", addr_family!(self.addr()));

        self.cloned  = None;
        self.running = false;

        info!("Persisting routing table on shutdown ...");
        if let Some(path) = self.store_path.take() {
            self.rt.borrow_mut().save(&path);
        }
        self.taskman.borrow_mut().cancel_all();
    }

    pub(crate) fn on_message(&mut self, msg: Rc<RefCell<Box<dyn Msg>>>) {
        match msg.borrow().kind() {
            Kind::Error    => self.on_error(msg.clone()),
            Kind::Request  => self.on_request(msg.clone()),
            Kind::Response => self.on_response(msg.clone()),
        };
        self.received(msg);
    }

    fn received(&mut self, msg: Rc<RefCell<Box<dyn Msg>>>) {
        let borrowed = msg.borrow();
        let from_id  = borrowed.id();
        let from_addr= borrowed.origin();

        if is_bogon_addr!(from_addr) {
            info!("Received a message from bogon address {}, ignored the potential
                  routing table operation", from_addr);
            return;
        }

        let call = borrowed.associated_call();
        if let Some(call) = call.as_ref() {
            // we only want remote nodes with stable ports in our routing table,
            // so apply a stricter check here
            if !call.borrow().matches_addr() {
                return;
            }
        }

        let mut found = false;
        if let Some(old) = self.rt.borrow().bucket_entry(from_id) {
            // this might happen if one node changes ports (broken NAT?) or IP address
            // ignore until routing table entry times out
            if old.borrow().ni().socket_addr() != from_addr {
                return;
            }
            found = true;
        }

        if let Some(known_id) = self.known_nodes.get(from_addr) {
            if known_id != from_id {
                if let Some(known_entry) = self.rt.borrow().bucket_entry(known_id) {
                    // It's happening under the following conditions:
                    // 1) a node with that address is in our routing table, and
                    // 2) the ID does not match our routing table entry
                    //
                    // That means we are certain that the node either changed its node ID or
                    // is engaging in ID-spoofing. In either case, we don't want it in our
                    // routing table.
                    warn!("force-removing routing table entry {} because ID-change was detected; new ID {}",
                        known_entry.borrow(), from_id);
                    self.rt.borrow_mut().remove(known_id);

                    // Might be a pollution attack, check other entries in the same bucket too in case
                    // random pings can't keep up with scrubbing.
                    let bucket = self.rt.borrow().bucket(known_id);
                    let name = format!("Checking bucket {} after ID change was detected", bucket.borrow().prefix());
                    self.rt().borrow_mut().try_ping_maintenance(PingOption::CheckAll, bucket, &name);
                    self.known_nodes.insert(from_addr.clone(), from_id.clone());
                    return;
                } else {
                    self.known_nodes.remove(from_addr);
                }
            }
        }
        self.known_nodes.insert(from_addr.clone(), from_id.clone());

        let entry = Rc::new(RefCell::new({
            let mut entry = KBucketEntry::with_ver(
                borrowed.id().clone(),
                from_addr.clone(),
                borrowed.ver(),
            );

            if let Some(call) = call {
                entry.signal_response();
                entry.merge_request_time(call.borrow().sent_time().clone());
            } else if !found {
                let call = {
                    use crate::core::msg::ping_req as req;
                    let msg = Box::new(req::Message::new());
                    Rc::new(RefCell::new(RpcCall::new(
                        entry.ni(),
                        self.dht(),
                        Rc::new(RefCell::new(msg as Box<dyn Msg>))
                    )))
                };
                call.borrow_mut().set_cloned(call.clone());
                self.server().borrow_mut().send_call(call);
            }
            entry
        }));
        self.rt.borrow_mut().put(entry);
    }

    fn on_request(&mut self, msg: Rc<RefCell<Box<dyn Msg>>>) {
        let borrowed = msg.borrow();
        let borrowed_deref = borrowed.deref();

        match borrowed_deref.method() {
            Method::Ping        => self.on_ping(borrowed_deref),
            Method::FindNode    => self.on_find_node(borrowed_deref),
            Method::FindValue   => self.on_find_value(borrowed_deref),
            Method::StoreValue  => self.on_store_value(borrowed_deref),
            Method::FindPeer    => self.on_find_peers(borrowed_deref),
            Method::AnnouncePeer=> self.on_announce_peer(borrowed_deref),
            Method::Unknown     => self.send_err(borrowed_deref, 203, "Invalid request method")
        }
    }

    fn on_response(&mut self, _: Rc<RefCell<Box<dyn Msg>>>) {}
    fn on_error(&mut self, msg: Rc<RefCell<Box<dyn Msg>>>) {
        let borrowed = msg.borrow();
        let downcasted = {
            use crate::core::msg::error_msg::Message;
            borrowed.as_any().downcast_ref::<Message>()
        }.unwrap();

        warn!("Error from {}/{} - {}:{}, txid {}",
            downcasted.origin(),
            version::normailized_version(downcasted.ver()),
            downcasted.code(),
            downcasted.msg(),
            downcasted.txid()
        );
    }

    fn send_err(&mut self, msg: &Box<dyn Msg>, code: i32, str: &str) {
        let msg = Rc::new(RefCell::new({
            use crate::core::msg::error_msg::Message as Message;
            let mut err = Box::new(Message::new(msg.method(), msg.txid()));
            err.set_remote(msg.id(), msg.origin());
            err.set_ver(version::ver());
            err.set_txid(msg.txid());
            err.with_msg(str);
            err.with_code(code);
            err as Box<dyn Msg>
        }));

        self.server().borrow_mut().send_msg(msg);
    }

    fn on_ping(&mut self, msg: &Box<dyn Msg>) {
        let msg = Rc::new(RefCell::new({
            use crate::core::msg::ping_rsp::Message as Message;
            let mut rsp = Box::new(Message::new());
            rsp.set_txid(msg.txid());
            rsp.set_remote(msg.id(), msg.origin());
            rsp as Box<dyn Msg>
        }));
        self.server().borrow_mut().send_msg(msg);
    }

    fn on_find_node(&mut self, msg: &Box<dyn Msg>) {
        use crate::core::msg::{
            find_node_req as req,
            find_node_rsp as rsp
        };

        let use_ipv4 = self.network().is_ipv4();
        let req = msg.as_any().downcast_ref::<req::Message>().unwrap();
        let rsp = Rc::new(RefCell::new({
            let mut msg = Box::new(rsp::Message::new());
            msg.set_remote(req.id(), req.origin());
            msg.set_txid(req.txid());

            if req.want4() && use_ipv4 {
                msg.populate_closest_nodes4({
                    let mut kns = KClosestNodes::new(
                        req.target(),
                        self.ni.clone(),
                        self.rt.clone(),
                        constants::MAX_ENTRIES_PER_BUCKET,
                    );
                    kns.fill(use_ipv4);
                    kns.as_nodes()
                });
            }
            if req.want6() && !use_ipv4 {
                msg.populate_closest_nodes6({
                    let mut kns = KClosestNodes::new(
                        req.target(),
                        self.ni.clone(),
                        self.rt.clone(),
                        constants::MAX_ENTRIES_PER_BUCKET,
                    );
                    kns.fill(!use_ipv4);
                    kns.as_nodes()
                });
            }
            if req.want_token() {
                let borrowed = unwrap!(self.tokman).borrow_mut();
                msg.populate_token({
                    borrowed.generate_token(
                        req.id(),
                        req.origin(),
                        req.target().as_ref()
                    )
                });
            }
            msg as Box<dyn Msg>
        }));

        self.server().borrow_mut().send_msg(rsp);
    }

    fn on_find_value(&mut self, msg: &Box<dyn Msg>) {
        use crate::core::msg::{
            find_value_req as req,
            find_value_rsp as rsp
        };

        let use_ipv4 = self.network().is_ipv4();
        let req = msg.as_any().downcast_ref::<req::Message>().unwrap();
        let rsp = Rc::new(RefCell::new({
            let mut msg = Box::new(rsp::Message::new());
            msg.set_remote(req.id(), req.origin());
            msg.set_txid(req.txid());

            let mut found_value = false;
            let value = unwrap!(self.storage).borrow_mut()
                .value(&req.target())
                .map_err(|e| error!("{}",e))
                .unwrap();

            if let Some(v) = value {
                if req.seq() < 0 || v.sequence_number() < 0
                    || req.seq() <= v.sequence_number()
                {
                    found_value = true;
                    msg.populate_value(Rc::new(v));
                }
            }

            if req.want4() && use_ipv4 && found_value {
                msg.populate_closest_nodes4({
                    let mut kns = KClosestNodes::new(
                        req.target(),
                        self.ni.clone(),
                        self.rt.clone(),
                        constants::MAX_ENTRIES_PER_BUCKET,
                    );
                    kns.fill(use_ipv4);
                    kns.as_nodes()
                });
            }

            if req.want6() && !use_ipv4 && found_value {
                msg.populate_closest_nodes6({
                    let mut kns = KClosestNodes::new(
                        req.target(),
                        self.ni.clone(),
                        self.rt.clone(),
                        constants::MAX_ENTRIES_PER_BUCKET,
                    );
                    kns.fill(!use_ipv4);
                    kns.as_nodes()
                });
            }

            if req.want_token() {
                let borrowed = unwrap!(self.tokman).borrow_mut();
                msg.populate_token({
                    borrowed.generate_token(
                        req.id(),
                        req.origin(),
                        req.target().as_ref()
                    )
                });
            }
            msg as Box<dyn Msg>
        }));

        self.server().borrow_mut().send_msg(rsp);
    }

    fn on_store_value(&mut self, msg: &Box<dyn Msg>) {
        use crate::core::msg::{
            store_value_req as req,
            store_value_rsp as rsp
        };

        let req = msg.as_any().downcast_ref::<req::Message>().unwrap();
        let value = req.value();
        let value_id = value.id();

        let tokman = unwrap!(self.tokman);
        let _valid = {
            tokman.borrow().verify_token(
                req.token(),
                req.id(),
                req.origin(),
                &value_id
            )
        };

        let valid = true; //TODO:
        if !valid {
            warn!("Received a store value request with invalid token from {}", req.origin());
            self.send_err(msg, 203, "Invalid token for store value request");
            return;
        }

        if !value.is_valid() {
            warn!("Received a store value request failed on verification from {}", req.origin());
            self.send_err(msg, 203, "Invalid value");
            return;
        }

        self.storage().borrow_mut()
            .put_value(&value, Some(0), Some(true), None)
            .map_err(|e| error!("{}",e))
            .unwrap();

        let rsp = Rc::new(RefCell::new({
            let mut msg = Box::new(rsp::Message::new());
            msg.set_remote(req.id(), req.origin());
            msg.set_txid(req.txid());
            msg as Box<dyn Msg>
        }));

        self.server().borrow_mut().send_msg(rsp);
    }

    fn on_find_peers(&mut self, msg: &Box<dyn Msg>) {
        use crate::core::msg::{
            find_peer_req as req,
            find_peer_rsp as rsp
        };

        let use_ipv4 = self.network().is_ipv4();
        let req = msg.as_any().downcast_ref::<req::Message>().unwrap();
        let rsp = Rc::new(RefCell::new({
            let mut msg = Box::new(rsp::Message::new());
            msg.set_remote(req.id(), req.origin());
            msg.set_txid(req.txid());

            let mut found_peers = false;
            let peers = unwrap!(self.storage).borrow_mut()
                .peers(&req.target(), 8)
                .map_err(|e| error!("{}",e))
                .unwrap();

            if peers.len() > 0 {
                found_peers = true;
                msg.populate_peers(peers.into_iter().collect());
            }

            if req.want4() && use_ipv4 && found_peers {
                msg.populate_closest_nodes4({
                    let mut kns = KClosestNodes::new(
                        req.target(),
                        self.ni.clone(),
                        self.rt.clone(),
                        constants::MAX_ENTRIES_PER_BUCKET,
                    );
                    kns.fill(use_ipv4);
                    kns.as_nodes()
                });
            }

            if req.want6() && !use_ipv4 && found_peers {
                msg.populate_closest_nodes6({
                    let mut kns = KClosestNodes::new(
                        req.target(),
                        self.ni.clone(),
                        self.rt.clone(),
                        constants::MAX_ENTRIES_PER_BUCKET,
                    );
                    kns.fill(!use_ipv4);
                    kns.as_nodes()
                });
            }

            if req.want_token() {
                let borrowed = unwrap!(self.tokman).borrow_mut();
                msg.populate_token({
                    borrowed.generate_token(
                        req.id(),
                        req.origin(),
                        req.target().as_ref()
                    )
                });
            }
            msg as Box<dyn Msg>
        }));

        self.server().borrow_mut().send_msg(rsp);
    }

    fn on_announce_peer(&mut self, msg: &Box<dyn Msg>) {
        use crate::core::msg::{
            announce_peer_req as req,
            announce_peer_rsp as rsp
        };

        let req = msg.as_any().downcast_ref::<req::Message>().unwrap();
        if is_bogon_addr!(msg.origin()) {
            info!("Received an announce peer request from bogon address {}, ignored ",
                msg.origin()
            );
        }

        let tokman = unwrap!(self.tokman);
        let peer = req.peer();
        let _valid = {
            tokman.borrow().verify_token(
                req.token(),
                req.id(),
                req.origin(),
                req.target()
            )
        };

        let valid = true; //TODO:
        if !valid {
            warn!("Received an announce peer request with invalid token from {}", req.origin());
            self.send_err(msg, 203,"Invalid token for ANNOUNCE PEER request");
            return;
        }

        if !peer.is_valid() {
            warn!("Received an announce peer request, but verification failed from {}", req.origin());
            self.send_err(msg, 203, "The peer is invalid peer");
            return;
        }

        debug!( "Received an announce peer request from {}, saving peer {}",
            req.origin(), req.target());

        self.storage().borrow_mut()
            .put_peer(&peer, Some(true), None)
            .map_err(|e| error!("{}", e))
            .unwrap();

        let rsp = Rc::new(RefCell::new({
            let mut msg = Box::new(rsp::Message::new());
            msg.set_remote(req.id(), req.origin());
            msg.set_txid(req.txid());
            msg as Box<dyn Msg>
        }));

        self.server().borrow_mut().send_msg(rsp);
    }

    pub(crate) fn on_timeout(&mut self, call: &RpcCall) {
        // Ignore the timeout if the DHT is stopped or the RPC server is offline
        if  !self.running ||
            !self.server().borrow().is_reachable() {
            return;
        }
        self.rt.borrow_mut().on_timeout(call.target_id());
    }

    pub(crate) fn on_send(&mut self, id: &Id) {
        if self.running {
            self.rt.borrow_mut().on_send(id);
        }
    }

    pub(crate) fn find_node<F>(&self,
        target: &Id,
        option: LookupOption,
        complete_fn: Rc<RefCell<F>>
    ) where F: FnMut(Option<NodeInfo>) + 'static {
        let result = Rc::new(RefCell::new({
            self.rt.borrow()
                .bucket_entry(&target)
                .map(|v| v.borrow().ni().clone())
        }));

        let task = Rc::new(RefCell::new({
            let mut task_ = Box::new(NodeLookupTask::new(
                Rc::new(target.clone()),
                self.dht()
            ));
            task_.set_name("LookupNode");

            let cloned = result.clone();
            task_.set_result_fn(move |_task, _ni| {
                if let Some(ni) = _ni {
                    *(cloned.borrow_mut()) = Some(ni.clone());
                }
                if option == LookupOption::Conservative {
                      _task.cancel()
                }
            });

            let cloned = result.clone();
            task_.add_listener(Box::new(move |_| {
                complete_fn.borrow_mut()(
                    cloned.borrow().as_ref().map(|v| v.deref().clone())
                );
            }));
            task_ as Box<dyn Task>
        }));

        self.taskman.borrow_mut().add(task);
    }

    pub(crate) fn find_value<F>(&self,
        value_id: Rc<Id>,
        option: LookupOption,
        complete_fn: Rc<RefCell<F>>
    ) where F: FnMut(Option<Value>) + 'static {
        let result = Rc::new(RefCell::new(None as Option<Value>));
        let task = Rc::new(RefCell::new({
            let cloned = result.clone();
            let mut task_ = Box::new(ValueLookupTask::new(self.dht(), value_id));
            task_.set_name("LookupValue");
            task_.with_expected_seq(-1);
            task_.set_result_fn(move |_task, _value| {
                if let Some(_v) = _value.as_ref() {
                    if cloned.borrow().is_some() {
                        if _v.is_mutable() && cloned.borrow().as_ref().unwrap().sequence_number() < _v.sequence_number() {
                            *(cloned.borrow_mut()) = Some(_v.deref().clone());
                        }
                    } else {
                        *(cloned.borrow_mut()) = Some(_v.deref().clone());
                    }
                }
                if option != LookupOption::Conservative {
                    if let Some(_v) = _value {
                        if !_v.is_mutable() {
                            _task.borrow_mut().cancel()
                        }
                    }
                }
            });

            let cloned = result.clone();
            task_.add_listener(Box::new(move |_| {
                complete_fn.borrow_mut()(cloned.borrow_mut().take());
            }));
            task_ as Box<dyn Task>
        }));

        self.taskman.borrow_mut().add(task);
    }

    pub(crate) fn store_value<F>(&self,
        value: &Value,
        complete_fn: Rc<RefCell<F>>
    ) where F: FnMut(Option<Vec<NodeInfo>>) + 'static {
        let mut task = Box::new(NodeLookupTask::new(
            Rc::new(value.id()),
            self.dht()
        ));
        task.set_name("LookupNode");
        task.set_want_token(true);

        let dht = self.dht();
        let taskman = self.taskman.clone();
        let v = Rc::new(value.clone());
        let complete_fn = complete_fn.clone();
        task.add_listener(Box::new(move |_task| {
            if _task.state() != State::Finished {
                return;
            }
            let downcasted = match _task.as_any().downcast_ref::<NodeLookupTask>() {
                Some(downcasted) => downcasted,
                None => return,
            };

            let closest_set = downcasted.closest_set();
            if closest_set.borrow().size() == 0 {
                // This should never happen
                warn!("!!! Value announce task not started because the node lookup task got the empty closest nodes.");
                complete_fn.borrow_mut()(Option::default());
                return;
            }

            let complete_fn = complete_fn.clone();
            let announce = Rc::new(RefCell::new({
                let mut nested = Box::new(ValueAnnounceTask::new(
                    dht.clone(),
                    closest_set.clone(),
                    v.clone()
                ));
                nested.set_name("ValueAnnounce");
                nested.add_listener(Box::new(move |_| {
                    let mut result = Vec::new();
                    for item in closest_set.borrow().entries().iter() {
                        result.push(item.borrow().ni().deref().clone());
                    }
                    complete_fn.borrow_mut()(Some(result));
                }));
                nested.set_name("Nested ValueAnnounce");
                nested as Box<dyn Task>
            }));

            _task.set_nested(announce.clone());
            taskman.borrow_mut().add(announce);
        }));

        self.taskman.borrow_mut().add(
            Rc::new(RefCell::new(task))
        );
    }

    pub(crate) fn find_peer<F>(&self,
        peer_id: Rc<Id>,
        expected: usize,
        option: LookupOption,
        complete_fn: Rc<RefCell<F>>
    ) where F: FnMut(Vec<PeerInfo>) + 'static {
        let peers = Rc::new(RefCell::new(Vec::new()));
        let dedup = Rc::new(RefCell::new(HashSet::new()));
        let cloned_peers = peers.clone();
        let mut task = Box::new(PeerLookupTask::new(self.dht(), peer_id));
        task.set_name("lookupPeer");
        task.set_result_fn(move |_task, mut _peers| {
            while let Some(item) = _peers.pop() {
                let hash = {
                    let mut s = DefaultHasher::new();
                    item.hash(&mut s);
                    s.finish()
                };
                if dedup.borrow_mut().insert(hash) {
                    cloned_peers.borrow_mut().push(item);
                }
            }
            if option != LookupOption::Conservative &&
                cloned_peers.borrow_mut().len() > expected {
                _task.borrow_mut().cancel()
            }
        });

        let cloned_peers = peers.clone();
        task.add_listener(Box::new(move |_| {
            let moved_value = {
                let mut borrowed = cloned_peers.borrow_mut();
                std::mem::take(&mut *borrowed)
            };
            complete_fn.borrow_mut()(moved_value);
        }));

        self.taskman.borrow_mut().add(
            Rc::new(RefCell::new(task))
        );
    }

    pub(crate) fn announce_peer<F>(&self,
        peer: &PeerInfo,
        complete_fn: Rc<RefCell<F>>
    ) where F: FnMut(Option<Vec<NodeInfo>>) + 'static {
        let mut task = Box::new(NodeLookupTask::new(
            Rc::new(peer.id().clone()),
            self.dht()
        ));
        task.set_name("LookupNode");
        task.set_want_token(true);

        let dht = self.dht();
        let taskman = self.taskman.clone();
        let p = Rc::new(peer.clone());
        let complete_fn = complete_fn.clone();
        task.add_listener(Box::new(move |_task| {
            if _task.state() != State::Finished {
                return;
            }

            let downcasted = match _task.as_any().downcast_ref::<NodeLookupTask>() {
                Some(downcasted) => downcasted,
                None => return,
            };

            let closest_set = downcasted.closest_set();
            if closest_set.borrow().size() == 0 {
                // This should never happen
                warn!("!!! Peer announce task not started because the node lookup task got the empty closest nodes.");
                complete_fn.borrow_mut()(Option::default());
                return;
            }

            let complete_fn = complete_fn.clone();
            let announce = Rc::new(RefCell::new({
                let mut nested = Box::new(PeerAnnounceTask::new(dht.clone(), closest_set.clone(), p.clone()));
                nested.set_name("PeerAnnounce");
                nested.add_listener(Box::new(move |_| {
                    let mut result = Vec::new();
                    for item in closest_set.borrow().entries().iter() {
                        result.push(item.borrow().ni().deref().clone());
                    }
                    complete_fn.borrow_mut()(Some(result));
                }));
                nested.set_name("Nested PeerAnnounce");
                nested as Box<dyn Task>
            }));

            _task.set_nested(announce.clone());
            taskman.borrow_mut().add(announce);
        }));

        self.taskman.borrow_mut().add(
            Rc::new(RefCell::new(task))
        )
    }
}
