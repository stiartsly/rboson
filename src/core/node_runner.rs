use std::rc::Rc;
use std::cell::RefCell;
use std::time::{Duration, SystemTime};
use std::sync::{Arc, Mutex};
use std::collections::LinkedList;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use log::{debug, info, warn, error};

use crate::{
    Id,
    Value,
    Network,
    NodeInfo,
    PeerInfo,
    JointResult,
    LookupOption,
    signature,
    Error,
};

use crate::core::{
    constants,
    dht::DHT,
    sqlite_storage::SqliteStorage,
    token_manager::TokenManager,
    server::{self, Server},
    crypto_cache::CryptoCache,
    bootstrap_channel::BootstrapChannel,
    data_storage::DataStorage
};

use crate::core::future::{
    Cmd,
    Command,
    FindNodeCmd,
    FindValueCmd,
    FindPeerCmd,
    StoreValueCmd,
    AnnouncePeerCmd,
    GetValueCmd,
    RemoveValueCmd,
    GetValueIdsCmd,
    GetPeerCmd,
    RemovePeerCmd,
    GetPeerIdsCmd,
};

pub(crate) struct NodeRunner {
    nodeid: Rc<Id>,

    data_dir: String,

    encryption_ctx: Arc<Mutex<CryptoCache>>,
    command_channel: Arc<Mutex<LinkedList<Command>>>,
    bootstr_channel: Arc<Mutex<BootstrapChannel>>,

    dht4: Option<Rc<RefCell<DHT>>>,
    dht6: Option<Rc<RefCell<DHT>>>,
    dht_num: i32,

    storage:  Rc<RefCell<dyn DataStorage>>,
    tokenman: Rc<RefCell<TokenManager>>,
    server:   Rc<RefCell<Server>>,

    cloned: Option<Rc<RefCell<NodeRunner>>>,
}

impl NodeRunner {
    pub(crate) fn new(
        data_dir: String,
        keypair: signature::KeyPair,
        addrs: JointResult<SocketAddr>,
        command_channel: Arc<Mutex<LinkedList<Command>>>,
        bootstr_channel: Arc<Mutex<BootstrapChannel>>,
        crypto_context: Arc<Mutex<CryptoCache>>
    ) -> Self {
        let nodeid = Rc::new(Id::from(keypair.to_public_key()));

        let mut dht_num = 0;
        let dht4 = addrs.v4().map(|addr| {
            let mut dht = DHT::new(nodeid.clone(), addr.clone());
            dht.enable_persistence(data_dir.clone() + "dht4.cache");
            dht_num += 1;
            dht
        });
        let dht6 = addrs.v6().map(|addr| {
            let mut dht = DHT::new(nodeid.clone(), addr.clone());
            dht.enable_persistence(data_dir.clone() + "dht6.cache");
            dht_num += 1;
            dht
        });

        Self {
            nodeid: nodeid.clone(),

            data_dir,
            encryption_ctx: crypto_context,

            command_channel,
            bootstr_channel,

            dht4: dht4.map(|v| Rc::new(RefCell::new(v))),
            dht6: dht6.map(|v| Rc::new(RefCell::new(v))),
            dht_num,

            storage:  Rc::new(RefCell::new(SqliteStorage::new())),
            tokenman: Rc::new(RefCell::new(TokenManager::new())),
            server:   Rc::new(RefCell::new(Server::new(nodeid))),

            cloned:   None,
        }
    }

    pub(crate) fn set_cloned(&mut self, runner: Rc<RefCell<NodeRunner>>) {
        self.cloned = Some(runner);
    }

    pub(crate) fn id(&self) -> Rc<Id> {
        self.nodeid.clone()
    }

    pub(crate) fn cloned(&self) -> Rc<RefCell<NodeRunner>> {
        self.cloned.as_ref().unwrap().clone()
    }

    fn persistent_announce(&mut self) {
        info!("Re-announce the persistent values and peers ...");

        let before = SystemTime::now().checked_sub(Duration::from_millis(
            constants::MAX_VALUE_AGE as u64 + constants::RE_ANNOUNCE_INTERVAL * 2
        )).unwrap();

        let result = self.storage.borrow_mut().persistent_values(&before)
            .map_err(|e| warn!("{}", e))
            .ok();

        let cloned_storage = self.storage.clone();
        let cloned_runner  = self.cloned();
        result.map(|values| {
            values.iter().for_each(|item| {
                let val_id = item.id();
                debug!("Reannouce the value {}",  &val_id);
                cloned_storage.borrow_mut().update_value_last_announce(&val_id)
                    .map_err(|e| warn!("{}", e))
                    .ok();

                cloned_runner.borrow().store_value(
                    Arc::new(Mutex::new(StoreValueCmd::new(item, false))),
                    false
                );
            })
        });

        let before = SystemTime::now().checked_sub(Duration::from_millis(
            constants::MAX_PEER_AGE as u64 + constants::RE_ANNOUNCE_INTERVAL * 2
        )).unwrap();

        let result = self.storage.borrow_mut().persistent_peers(&before)
            .map_err(|e| warn!("{}", e))
            .ok();

        let cloned_storage = self.storage.clone();
        let cloned_runner  = self.cloned();
        result.map(|peers| {
            peers.iter().for_each(|item| {
                debug!("Reannouce the peers {}",  item.id());
                cloned_storage.borrow_mut().update_peer_last_announce(item.id(), item.origin())
                    .map_err(|e| warn!("{}", e))
                    .ok();

                cloned_runner.borrow().announce_peer(
                    Arc::new(Mutex::new(AnnouncePeerCmd::new(item, false))),
                    false
                );
            })
        });
    }

    pub(crate) fn start(&mut self) -> Result<(), Error> {
        // Prepare SQlite storage
        let path = self.data_dir.clone() + "node.db";
        self.storage.borrow_mut().open(path.as_str())?;

        // Start IPv4 DHT if it exists
        self.dht4.as_ref().map(|dht| dht.borrow_mut()
            .set_field(self.server.clone())
            .set_field(self.storage.clone())
            .set_field(self.tokenman.clone())
            .set_field(dht.clone())
            .start()
            .map(|addr| info!("Started DHT node on ipv4 address: {}", addr))
        );

        // Start IPv6 DHT if it exists
        self.dht6.as_ref().map(|dht| dht.borrow_mut()
            .set_field(self.server.clone())
            .set_field(self.storage.clone())
            .set_field(self.tokenman.clone())
            .set_field(dht.clone())
            .start()
            .map(|addr| info!("Started DHT node on ipv6 address: {}", addr))
        );

        let scheduler = self.server.borrow().scheduler();

        // Handle encryption context expiration.
        let ctx = self.encryption_ctx.clone();
        scheduler.borrow_mut().add(move || {
            ctx.lock().unwrap().expire();
        }, 2000, constants::EXPIRED_CHECK_INTERVAL);

        // Handle SQlite storage expiration.
        let storage = self.storage.clone();
        scheduler.borrow_mut().add(move || {
            storage.borrow_mut().expire();
        }, 1000, constants::STORAGE_EXPIRE_INTERVAL);

        let cloned = self.cloned();
        scheduler.borrow_mut().add(move || {
            cloned.borrow_mut().persistent_announce();
        }, 1000, constants::RE_ANNOUNCE_INTERVAL);

        // Check incomming bootstrap nodes.
        let chan = self.bootstr_channel.clone();
        let dht4 = self.dht4.as_ref().map(|v| v.clone());
        let dht6 = self.dht6.as_ref().map(|v| v.clone());

        scheduler.borrow_mut().add(move || {
            let mut channel = chan.lock().unwrap();
            channel.pop_all(|item| {
                let ni = Rc::new(item);
                dht4.as_ref().map(|dht| {
                    dht.borrow_mut().add_bootstrap_node(ni.clone());
                });
                dht6.as_ref().map(|dht| {
                    dht.borrow_mut().add_bootstrap_node(ni.clone());
                });
            });
        }, 100, constants::DHT_UPDATE_INTERVAL);

        // Check incomming commands from outer Node.
        let chan = self.command_channel.clone();
        let node = self.cloned();
        scheduler.borrow_mut().add(move || {
            let mut channel = chan.lock().unwrap();
            while let Some(cmd) = channel.pop_front() {
                let borrowed = node.borrow();
                match cmd {
                    Command::FindNode(c)    => borrowed.find_node(c),
                    Command::FindValue(c)   => borrowed.find_value(c),
                    Command::FindPeer(c)    => borrowed.find_peer(c),
                    Command::StoreValue(c)  => borrowed.store_value(c, true),
                    Command::AnnouncePeer(c)=> borrowed.announce_peer(c, true),
                    Command::GetValue(c)    => borrowed.get_value(c),
                    Command::RemoveValue(c) => borrowed.remove_value(c),
                    Command::GetValueIds(c) => borrowed.get_value_ids(c),
                    Command::GetPeer(c)     => borrowed.get_peer(c),
                    Command::RemovePeer(c)  => borrowed.remove_peer(c),
                    Command::GetPeerIds(c)  => borrowed.get_peer_ids(c),
                }
            }
        }, 100, 100);

        Ok(())
    }

    pub(crate) fn stop(&mut self) {
        self.server.borrow_mut().stop();
        self.storage.borrow_mut().close();

        self.dht4.take().map(|dht| {
            dht.borrow_mut().stop();
            info!("Stopped DHT node on ipv4 address: {}", dht.borrow().addr());
        });
        self.dht6.take().map(|dht| {
            dht.borrow_mut().stop();
            info!("Stopped DHT node on ipv6 address: {}", dht.borrow().addr());
        });
    }

    fn find_node(&self, cmd: Arc<Mutex<FindNodeCmd>>) {
        let mut locked = cmd.lock().unwrap();
        let mut found = JointResult::new();

        self.dht4.as_ref().map(|dht| dht.borrow().rt()
            .borrow()
            .bucket_entry(locked.target())
            .map(|v| found.set_value(
                Network::IPv4,
                v.borrow().ni().deref().clone()
            ))
        );
        self.dht6.as_ref().map(|dht| dht.borrow().rt()
            .borrow()
            .bucket_entry(locked.target())
            .map(|v| found.set_value(
                Network::IPv6,
                v.borrow().ni().deref().clone()
            ))
        );

        let option = locked.option().clone();
        if option == LookupOption::Arbitrary && found.has_value() {
            locked.complete(Ok(found));
            return;
        }

        let cloned = cmd.clone();
        let ndhts = self.dht_num;
        let found = Rc::new(RefCell::new(found));
        let completion = Rc::new(RefCell::new(0));
        let complete_fn = Rc::new(RefCell::new(
            move |ni: Option<NodeInfo> | {
                *completion.borrow_mut() += 1;
                ni.map(|ni| found.borrow_mut().set_value(
                    Network::from(ni.socket_addr()),
                    ni
                ));

                if (option == LookupOption::Optimistic && found.borrow().has_value()) ||
                    *completion.borrow() >= ndhts {
                        // Assuming this is the only Rc instance pointing to the value,
                        // then we can move out the value.

                        let jresult = match Rc::try_unwrap(found.clone()) {
                            Ok(v) => v.into_inner(),
                            Err(_) => found.borrow().clone()
                        };
                        cloned.lock().unwrap().complete(Ok(jresult));
                }
            }
        ));

        self.dht4.as_ref().map(|dht| dht.borrow().find_node(
            locked.target(), option, complete_fn.clone()
        ));
        self.dht6.as_ref().map(|dht| dht.borrow().find_node(
            locked.target(), option, complete_fn.clone()
        ));
    }

    fn find_value(&self, cmd: Arc<Mutex<FindValueCmd>>) {
        let mut locked = cmd.lock().unwrap();
        let value_id = locked.value_id();

        let result = self.storage.borrow_mut().value(value_id);
        if let Err(e) = result {
            error!("Query value from local storage error: {}", e);
            locked.complete(Err(e));
            return;
        }

        let found = result.unwrap();
        let option = locked.option().clone();
        if let Some(value) = found.as_ref() {
            if option == LookupOption::Arbitrary || !value.is_mutable() {
                locked.complete(Ok(Some(value.clone())));
                return;
            }
        }

        let cloned_cmd = cmd.clone();
        let cloned_storage = self.storage.clone();
        let ndhts = self.dht_num;
        let found = Rc::new(RefCell::new(found));
        let completion = Rc::new(RefCell::new(0));
        let complete_fn = Rc::new(RefCell::new(move |mut _value: Option<Value> | {

            *completion.borrow_mut() += 1;
            _value.take().map(|v| {
                if  found.borrow().is_none() || !v.is_mutable() ||
                    found.borrow().as_ref().unwrap().sequence_number() < v.sequence_number() {
                    *found.borrow_mut() = Some(v);
                }
            });

            if (option == LookupOption::Optimistic && _value.is_some()) ||
                *completion.borrow() >= ndhts {

                found.borrow().as_ref().map(|v|
                    cloned_storage.borrow_mut()
                        .put_value_and_announce(&v, true)
                        .map_err(|e| error!("Perisist value in local storage failed {}", e))
                        .ok()
                );

                let value = match Rc::try_unwrap(found.clone()) {
                    Ok(v) => v.into_inner(),
                    Err(_) => found.borrow().clone()
                };
                cloned_cmd.lock().unwrap().complete(Ok(value))
            };
        }));

        self.dht4.as_ref().map(|dht| dht.borrow().find_value(
            Rc::new(value_id.clone()), option, complete_fn.clone()
        ));
        self.dht6.as_ref().map(|dht| dht.borrow().find_value(
            Rc::new(value_id.clone()), option, complete_fn.clone()
        ));
    }

    fn find_peer(&self, cmd: Arc<Mutex<FindPeerCmd>>) {
        let mut locked = cmd.lock().unwrap();
        let peer_id = locked.peer_id();

        let mut result = self.storage.borrow_mut().peers(
            locked.peer_id(),
            locked.expected_num()
        );

        if let Err(e) = result {
            error!("Query peer information from local storage error: {}", e);
            locked.complete(Err(e));
            return;
        }

        let mut dedup = HashSet::new();
        let mut found = Vec::new();

        while let Some(item) = result.as_mut().unwrap().pop() {
            let hash = {
                let mut s = DefaultHasher::new();
                item.hash(&mut s);
                s.finish()
            };
            if dedup.insert(hash) {
                found.push(item);
            }
        }

        let option = locked.option().clone();
        let expect = locked.expected_num();

        if option == LookupOption::Arbitrary && expect > 0 && found.len() >= expect {
            locked.complete(Ok(found));
            return;
        }

        let cloned_cmd = cmd.clone();
        let cloned_storage = self.storage.clone();
        let ndhts = self.dht_num;
        let found = Rc::new(RefCell::new(found));
        let completion = Rc::new(RefCell::new(0));
        let complete_fn = Rc::new(RefCell::new(move |mut _peers: Vec<PeerInfo> | {
            *completion.borrow_mut() += 1;
            while let Some(item) = _peers.pop() {
                let hash = {
                    let mut s = DefaultHasher::new();
                    item.hash(&mut s);
                    s.finish()
                };
                if  dedup.insert(hash) {
                    found.borrow_mut().push(item);
                }
            }

            cloned_storage.borrow_mut().put_peers(&found.borrow())
                .map_err(|e| error!("Perisist peers in local storage failed {}", e))
                .ok();

            if *completion.borrow() >= ndhts {
                let peers = match Rc::try_unwrap(found.clone()) {
                    Ok(v) => v.into_inner(),
                    Err(_) => found.borrow().clone()
                };
                cloned_cmd.lock().unwrap().complete(Ok(peers));
            }
        }));

        self.dht4.as_ref().map(|dht| dht.borrow().find_peer(
            Rc::new(peer_id.clone()), expect, option, complete_fn.clone()
        ));
        self.dht6.as_ref().map(|dht| dht.borrow().find_peer(
            Rc::new(peer_id.clone()), expect, option, complete_fn.clone()
        ));
    }

    fn store_value(&self, cmd: Arc<Mutex<StoreValueCmd>>, persistence_forced: bool) {
        let mut locked = cmd.lock().unwrap();
        assert!(locked.value().is_valid());

        if persistence_forced {
            self.storage.borrow_mut().put_value_and_announce(
                locked.value(),
                locked.persistent()
            ).map_err(|e| {
                error!("Failed to persist value to local SQLite storage: {}", e);
                locked.complete(Err(e));
            }).ok();
        }

        let num_dhts = self.dht_num;
        let cloned_cmd = cmd.clone();
        let completion = Rc::new(RefCell::new(0));
        let complete_fn = Rc::new(RefCell::new(move |_| {
            *completion.borrow_mut() += 1;
            if *completion.borrow() >= num_dhts {
                cloned_cmd.lock().unwrap().complete(Ok(()))
            }
        }));

        self.dht4.as_ref().map(|dht| dht.borrow().store_value(
            locked.value(), complete_fn.clone()
        ));
        self.dht6.as_ref().map(|dht| dht.borrow().store_value(
            locked.value(), complete_fn.clone()
        ));
    }

    fn announce_peer(&self, cmd: Arc<Mutex<AnnouncePeerCmd>>, persistence_forced: bool) {
        let mut locked = cmd.lock().unwrap();
        assert!(locked.peer().is_valid());

        if persistence_forced {
            self.storage.borrow_mut().put_peer_and_announce(
                locked.peer(),
                locked.persistent()
            ).map_err(|e| {
                error!("Failed to persist peer to SQLite storage: {}", e);
                locked.complete(Err(e));
            }).ok();
        }

        let num_dhts = self.dht_num;
        let cloned_cmd = cmd.clone();
        let completion = Rc::new(RefCell::new(0));
        let complete_fn = Rc::new(RefCell::new(move |_|{
            *completion.borrow_mut() += 1;
            if *completion.borrow() >= num_dhts {
                cloned_cmd.lock().unwrap().complete(Ok(()))
            }
        }));

        self.dht4.as_ref().map(|dht| dht.borrow().announce_peer(
            locked.peer(), complete_fn.clone()
        ));
        self.dht6.as_ref().map(|dht| dht.borrow().announce_peer(
            locked.peer(), complete_fn.clone()
        ));
    }

    fn get_value(&self, cmd: Arc<Mutex<GetValueCmd>>) {
        let mut locked = cmd.lock().unwrap();
        self.storage.borrow_mut().value(locked.value_id())
            .map(|value| {
                locked.complete(Ok(value))
            }).map_err(|e| {
                locked.complete(Err(e))
            }).ok();

    }

    fn remove_value(&self, cmd: Arc<Mutex<RemoveValueCmd>>) {
        let mut locked = cmd.lock().unwrap();
        self.storage.borrow_mut().remove_value(locked.value_id())
            .map(|_| {
                locked.complete(Ok(()))
            }).map_err(|e| {
                locked.complete(Err(e))
            }).ok();
    }

    fn get_value_ids(&self, cmd: Arc<Mutex<GetValueIdsCmd>>) {
        let mut locked = cmd.lock().unwrap();
        self.storage.borrow_mut().value_ids()
            .map(|ids| {
                locked.complete(Ok(ids))
            }).map_err(|e| {
                locked.complete(Err(e))
            }).ok();
    }

    fn get_peer(&self, cmd: Arc<Mutex<GetPeerCmd>>) {
        let mut locked = cmd.lock().unwrap();
        self.storage.borrow_mut().peer(locked.peer_id(), &self.id())
            .map(|peer| {
                locked.complete(Ok(peer))
            }).map_err(|e| {
                locked.complete(Err(e))
            }).ok();
    }

    fn remove_peer(&self, cmd: Arc<Mutex<RemovePeerCmd>>) {
        let mut locked = cmd.lock().unwrap();
        self.storage.borrow_mut().remove_peer(locked.peer_id(), &self.id())
            .map(|_| {
                locked.complete(Ok(()))
            }).map_err(|e| {
                locked.complete(Err(e))
            }).ok();
    }

    fn get_peer_ids(&self, cmd: Arc<Mutex<GetPeerIdsCmd>>) {
        let mut locked = cmd.lock().unwrap();
        self.storage.borrow_mut().peer_ids()
            .map(|ids| {
                locked.complete(Ok(ids))
            }).map_err(|e| {
                locked.complete(Err(e))
            }).ok();
    }

    pub(crate) fn encrypt_into(&self,
        recipient: &Id,
        plain: &[u8]
    ) -> Result<Vec<u8>, Error> {
        let mut ctx = self.encryption_ctx.lock().unwrap();
        ctx.get(recipient).lock().unwrap().ctx_mut().encrypt_into(plain)
    }

    pub(crate) fn decrypt_into(&self,
        sender: &Id,
        cipher: &[u8]
    ) -> Result<Vec<u8>, Error> {
        let mut ctx = self.encryption_ctx.lock().unwrap();
        ctx.get(sender).lock().unwrap().ctx_mut().decrypt_into(cipher)
    }
}

pub(crate) fn run_loop(runner: Rc<RefCell<NodeRunner>>,  quit: Arc<Mutex<bool>>) {
    let server = runner.borrow().server.clone();
    let dht4 = runner.borrow().dht4.as_ref().map(|v| v.clone());
    let dht6 = runner.borrow().dht6.as_ref().map(|v| v.clone());

    let mut to_quit = false;

    server.borrow_mut().start();
    runner.borrow_mut().start().err().map(|e| {
        error!("{}", e);
        to_quit = true;
    });

    if !to_quit {
        _ = server::run_loop(
            runner.clone(),
            server.clone(),
            dht4,
            dht6,
            quit.clone()
        ).map_err(|err| {
            error!("Internal error: {}.", err);
        });
    }

    runner.borrow_mut().stop();
    server.borrow_mut().stop();

    // notify the main thread about any abnormal or normal termination.
    let mut _quit = quit.lock().unwrap();
    if !*_quit {
        *_quit = true;
    }
    drop(_quit);
}
