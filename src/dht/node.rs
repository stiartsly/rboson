use std::ptr;
use std::io::Read;
use std::time::Duration;
use std::{fs, fs::File, io::Write};
use std::sync::{Arc, Mutex};
use futures::stream::{FuturesUnordered};
use tokio::task;
use log::{warn, error, info};

use crate::{
    CryptoIdentity,
    create_dirs,
    Id,
    Result,
    NodeInfo,
    PeerInfo,
    Value,
    Network,
    signature,
    JointResult,
    Identity,
    CryptoContext,
    core::logger,
    errors::{
        StateError,
        IOError,
        DBError,
        NetworkError,
        ArgumentError
    }
};

use crate::dht::{
    cfg::node_config::NodeConfig,
    node_status::NodeStatus,
    LookupOption,
    dht::DHT,
    eligible_value::EligibleValue,
    eligible_peers::EligiblePeers,
    cached_identity::CachedIdentity,
    token_manager::TokenManager,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
    errors::{
        SeqNotExpected,
        SeqNotMonotonic,
        NotOwnerError,
        ImmutableSubstitutionError
    }
};

pub struct Node {
    cfg: Box<dyn NodeConfig>,

    identity: CachedIdentity,

    option: Mutex<LookupOption>,
    status: Mutex<NodeStatus>,
    storage_path: String,

    dht4: Mutex<Option<Arc<Mutex<DHT>>>>,
    dht6: Mutex<Option<Arc<Mutex<DHT>>>>,

    storage : Arc<Mutex<Box<dyn DataStorage>>>,
    tokenman: Arc<Mutex<TokenManager>>,
}

impl Node {
    pub fn new(cfg: Box<dyn NodeConfig>) -> Result<Self> {
        Self::check_config(&cfg)?;
        logger::setup(cfg.as_ref().log_level(), cfg.as_ref().log_file().as_deref());
        logger::disable_console_output();

        #[cfg(feature = "devp")]
        info!("DHT node running in development mode!!!");

        let path = {
            let mut path = cfg.data_dir().to_string();
            if path.is_empty() {
                path.push_str(".")
            }
            if !path.ends_with("/") {
                path.push_str("/");
            }
            path
        };

        let identity = CachedIdentity::new({
            let kp = signature::KeyPair::from(cfg.private_key());
            CryptoIdentity::from_keypair(kp)
        });

        let id_path = path.clone() + "id";
        _ = store_nodeid(&id_path, identity.id()).map_err(|e| {
            error!("Persisting nodeid data error {}, skipped", e); e
        });

        info!("Current Node id: {}", identity.id());

        Ok( Self {
            cfg,
            identity,

            storage_path: path.clone(),

            option: Mutex::new(LookupOption::Conservative),
            status: Mutex::new(NodeStatus::Stopped),

            dht4: Mutex::new(None),
            dht6: Mutex::new(None),

            storage: Arc::new(Mutex::new(Box::new(SqliteStorage::new()))),
            tokenman: Arc::new(Mutex::new(TokenManager::new()))
        })
    }

    fn check_config(cfg: &Box<dyn NodeConfig>) -> Result<()> {
        if cfg.host4().is_none() && cfg.host6().is_none() {
            return Err(ArgumentError::new(
                "At least one host/address must be specified".to_string()
            ));
        }

        if cfg.bootstrap_nodes().is_empty() {
            warn!("No bootstrap nodes are configured");
        }

        let data_dir = cfg.data_dir();
        if !data_dir.is_empty() {
            let path = std::path::Path::new(data_dir);
            if path.exists() {
                if !path.is_dir() {
                    error!("Data path {} is not a directory", data_dir);
                    return Err(ArgumentError::new(format!(
                        "Data path {} is not a directory",
                        data_dir
                    )));
                }
            } else {
                create_dirs(data_dir).map_err(|e| {
                    error!("Data path {} can not be created", data_dir);
                    ArgumentError::new(format!(
                        "Data path {} can not be created: {}",
                        data_dir,
                        e
                    ))
                })?;
            }
        }
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        if *self.status.lock().unwrap() != NodeStatus::Stopped {
            return Ok(());
        }
        *self.status.lock().unwrap() = NodeStatus::Initializing;

        let storage_path = format!("{}node.db", self.storage_path);
        {
            let mut storage = self.storage.lock().unwrap();
            storage.open(&storage_path)?;
            storage.initialize(
                Duration::from_millis(120 * 60 * 1000),
                Duration::from_millis(120 * 60 * 1000),
            )?;
        }

        if let Some(host4) = self.cfg.host4() {
            let dht = Arc::new(Mutex::new(DHT::new(
                self.identity.identity(),
                Network::IPv4,
                host4.to_string(),
                self.cfg.port(),
                Some(format!("{}dht4.cache", self.storage_path)),
                self.cfg.bootstrap_nodes().to_vec(),
                self.storage.clone(),
                self.tokenman.clone(),
            )?));

            dht.lock().unwrap().set_cloned(dht.clone());
            if let Err(err) = dht.lock().unwrap().deploy().await {
                *self.status.lock().unwrap() = NodeStatus::Stopped;
                return Err(err);
            }
            *self.dht4.lock().unwrap() = Some(dht);
        }

        if let Some(host6) = self.cfg.host6() {
            let dht = Arc::new(Mutex::new(DHT::new(
                self.identity.identity(),
                Network::IPv6,
                host6.to_string(),
                self.cfg.port(),
                Some(format!("{}dht6.cache", self.storage_path)),
                self.cfg.bootstrap_nodes().to_vec(),
                self.storage.clone(),
                self.tokenman.clone(),
            )?));

            dht.lock().unwrap().set_cloned(dht.clone());
            if let Err(err) = dht.lock().unwrap().deploy().await {
                *self.status.lock().unwrap() = NodeStatus::Stopped;
                return Err(err);
            }
            *self.dht6.lock().unwrap() = Some(dht);
        }

        *self.status.lock().unwrap() = NodeStatus::Running;
        info!("Kademlia node started.");
        Ok(())
    }

    /*
    protected Future<Void> deploy() {
		tokenManager = new TokenManager();

		String storageURI = config.databaseUri();
		// fix the sqlite database file location
		if (storageURI.startsWith("jdbc:sqlite:")) {
			Path dbFile = Path.of(storageURI.substring("jdbc:sqlite:".length()));
			if (!dbFile.isAbsolute())
				storageURI = "jdbc:sqlite:" + config.dataDir().resolve(dbFile).toAbsolutePath();
		}
		storage = DataStorage.create(storageURI, config.databasePoolSize(), config.databaseSchemaName());

		// TODO: empty blacklist for now
		blacklist = Blacklist.empty();

		ConnectionStatusListener listener = new ConnectionStatusListener() {
			@Override
			public void statusChanged(Network network, ConnectionStatus newStatus, ConnectionStatus oldStatus) {
				if (connectionStatusListener != null)
					runOnContext(unused -> connectionStatusListener.statusChanged(network, newStatus, oldStatus));
			}

			@Override
			public void connecting(Network network) {
				if (connectionStatusListener != null)
					runOnContext(unused -> connectionStatusListener.connecting(network));
			}

			@Override
			public void connected(Network network) {
				if (connectionStatusListener != null)
					runOnContext(unused -> connectionStatusListener.connected(network));
			}

			@Override
			public void disconnected(Network network) {
				if (connectionStatusListener != null)
					runOnContext(unused -> connectionStatusListener.disconnected(network));
			}
		};

		return storage.initialize(vertx, MAX_VALUE_AGE, MAX_PEER_AGE).compose(unused -> {
			ArrayList<Future<Void>> futures = new ArrayList<>(2);
			if (config.host4() != null) {
				dht4 = new DHT(identity, Network.IPv4, config.host4(), config.port(), config.bootstrapNodes(),
						storage, config.dataDir().resolve("dht4.cache"),
						tokenManager, blacklist, config.enableSuspiciousNodeDetector(),
						config.enableSpamThrottling(), null, config.enableDeveloperMode());

				dht4.setConnectionStatusListener(listener);

				Future<Void> future = vertx.deployVerticle(dht4).andThen(ar -> {
					if (ar.failed())
						dht4 = null;
				}).mapEmpty();

				futures.add(future);
			}

			if (config.host6() != null) {
				dht6 = new DHT(identity, Network.IPv6, config.host6(), config.port(), config.bootstrapNodes(),
						storage, config.dataDir().resolve("dht6.cache"),
						tokenManager, blacklist, config.enableSuspiciousNodeDetector(),
						config.enableSpamThrottling(), null, config.enableDeveloperMode());

				dht6.setConnectionStatusListener(listener);

				Future<Void> future = vertx.deployVerticle(dht6).andThen(ar -> {
					if (ar.failed())
						dht6 = null;
				}).mapEmpty();
				futures.add(future);
			}

			return Future.all(futures);
		}).andThen(ar -> {
			if (ar.succeeded()) {
				long timer = vertx.setPeriodic(30_000, STORAGE_EXPIRE_INTERVAL, unused -> storage.purge());
				timers.add(timer);

				timer = vertx.setPeriodic(60_000, RE_ANNOUNCE_INTERVAL, unused -> persistentAnnounce());
				timers.add(timer);

				timer = vertx.setPeriodic(TokenManager.TOKEN_TIMEOUT, TokenManager.TOKEN_TIMEOUT, unused ->
						tokenManager.updateTokenTimestamps()
				);
				timers.add(timer);

				running = true;
				log.info("Kademlia node started.");
			} else {
				undeploy();
				log.error("Failed to start Kademlia node.", ar.cause());
			}
		}).mapEmpty();
	}
     */
/*
    pub fn start(&self) {
        let status_ptr: *mut NodeStatus = &mut *(self.status.lock().unwrap());
        unsafe {
            if ptr::read_volatile(status_ptr)
                != NodeStatus::Stopped {
                return;
            }
            ptr::write_volatile(status_ptr,
                NodeStatus::Initializing
            );
        }

        info!("DHT node <{}> is starting...", self.nodeid);

        let path    = self.storage_path.clone();
        let keypair = self.signature_keypair.clone();
        let addrs   = self.addrs.clone();
        let bootstr = self.bootstr_channel.clone();
        let ctx     = self.crypto_context.clone();
        let quit    = self.quit.clone();
        let thread  = thread::spawn(move || {
            let runner = Rc::new(RefCell::new(NodeRunner::new(
                path,
                keypair,
                addrs,
                bootstr,
                ctx
            )));

            runner.borrow_mut().set_cloned(runner.clone());
            node_runner::run_loop(
                runner,
                quit.clone()
            );
        });

        *self.thread.lock().unwrap() = Some(thread);
        unsafe {
            ptr::write_volatile(status_ptr,
                NodeStatus::Running
            );
        }
    }
    */

/*
    pub fn stop(&self) {
        let status_ptr: *mut NodeStatus = &mut (*self.status.lock().unwrap());
        unsafe {
            if ptr::read_volatile(status_ptr)
                == NodeStatus::Stopped {
                return;
            }
            ptr::write_volatile(
                status_ptr,
                NodeStatus::Stopped
            );
        }

        info!("DHT node <{}> stopping...", self.nodeid);

        // Check for abnormal termination in the spawned thread.
        // If the thread is still running, then notify it to abort.
        let mut quit = self.quit.lock().unwrap();
        if !*quit {
            *quit = true;
        }
        drop(quit);

        self.thread.lock().unwrap().take().unwrap().join().expect("Join thread error");
        *self.thread.lock().unwrap() = None;

        info!("DHT node {} stopped", self.nodeid);
        logger::teardown();
    }

    */

    pub fn id(&self) -> &Id {
        Identity::id(self)
    }

    pub fn node_info(&self) -> JointResult<NodeInfo> {
        let mut result = JointResult::new();
        if let Some(dht) = self.dht4.lock().unwrap().as_ref() {
            result.set_value(
                Network::IPv4,
                dht.lock().unwrap().ni().clone()
            );
        }
        if let Some(dht) = self.dht6.lock().unwrap().as_ref() {
            result.set_value(
                Network::IPv6,
                dht.lock().unwrap().ni().clone()
            );
        }
        result
    }

    pub fn version(&self) -> String {
        unimplemented!()
    }

    pub fn set_default_lookup_option(&self, option: LookupOption) {
        *self.option.lock().unwrap() = option;
    }

    pub fn default_lookup_option(&self) -> LookupOption {
        self.option.lock().unwrap().clone()
    }

    pub async fn stop(&self) -> Result<()> {
        //unimplemented!()
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        let status_ptr: *const NodeStatus = &(*self.status.lock().unwrap());
        unsafe {
            ptr::read_volatile(status_ptr) == NodeStatus::Running
        }
    }

    fn check_running(&self) -> Result<()> {
        match self.is_running() {
            true => Ok(()),
            false => Err(StateError::new("Node is not running".to_string()))
        }
    }

    pub async fn bootstrap_one(&self,  node: &NodeInfo) -> Result<()> {
        self.bootstrap(&[node.clone()]).await
    }

    pub async fn bootstrap(&self, nodes: &[NodeInfo]) -> Result<()> {
        if nodes.is_empty() {
            return Err(ArgumentError::new("Bootstrap nodes cannot be empty".to_string()));
        }
        self.check_running()?;

        let mut futs = FuturesUnordered::new();
        futs.push({
            let nodes = nodes.to_vec();
            let dht4 = self.dht4.lock().unwrap().clone();
            task::spawn_local(async move {
                if let Some(dht) = dht4 {
                    DHT::bootstrap(dht, nodes).await;
                }
            })
        });
        futs.push({
            let nodes = nodes.to_vec();
            let dht6 = self.dht6.lock().unwrap().clone();
            task::spawn_local(async move {
                if let Some(dht) = dht6 {
                    DHT::bootstrap(dht, nodes).await;
                }
            })
        });
        for handle in futs.iter_mut() {
            handle.await.map_err(|e|
                StateError::new(format!("Spawn bootstrap task error: {}", e))
            )?;
        }
        Ok(())
    }

    pub async fn find_node(
        &self,
        target: &Id,
        lookup_option: Option<LookupOption>
    ) -> Result<JointResult<NodeInfo>> {
        self.check_running()?;

        let option = lookup_option.unwrap_or(self.default_lookup_option());
        let futures = FuturesUnordered::new();
        futures.push({
            let dht = self.dht4.lock().unwrap().clone();
            let target = target.clone();
            task::spawn_local(async move {
                if let Some(dht) = dht {
                    DHT::find_node(dht, &target, option).await
                } else {
                    Ok(None)
                }
            })
        });
        futures.push({
            let dht = self.dht6.lock().unwrap().clone();
            let target = target.clone();
            task::spawn_local(async move {
                if let Some(dht) = dht {
                    DHT::find_node(dht, &target, option).await
                } else {
                    Ok(None)
                }
            })
        });

        let mut jresult = JointResult::<NodeInfo>::new();
        for handle in futures.into_iter() {
            let result = handle.await.map_err(|e| {
                StateError::new(format!("Spawn findNode task error: {}", e))
            })?;
            if let Some(ni) = result? {
                jresult.set_value(ni.network(), ni);
            }
        }
        Ok(jresult)
    }

    pub async fn find_value(&self,
        value_id: &Id,
        expected_sequence_number: i32,
        option: Option<LookupOption>
    ) -> Result<Option<Value>> {
        if expected_sequence_number < -1 {
            return Err(ArgumentError::new(format!("Invalid expected sequence number: {}",
                expected_sequence_number
            )));
        }
        self.check_running()?;

        let option = option.unwrap_or(self.default_lookup_option());
        let dht4 = self.dht4.lock().unwrap().clone();
        let dht6 = self.dht6.lock().unwrap().clone();
        let storage = self.storage.clone();
        let value_id = value_id.clone();

        task::spawn_local(async move {
            let mut eligible = EligibleValue::new(
                value_id,
                expected_sequence_number
            );
            let value = storage.lock().unwrap().get_value(&value_id)?;
            if let Some(value) = value {
                let is_mutable_val = value.is_mutable();
                eligible.update(value, false);
                if !is_mutable_val {
                    return Ok(eligible)
                }
                if option != LookupOption::Conservative && !eligible.is_empty() {
                    return Ok(eligible)
                }
            }

            let result = Self::do_find_value(
                value_id,
                expected_sequence_number,
                option,
                dht4,
                dht6
            ).await?;
            if let Some(v) = result {
                eligible.update(v, true);
            }
            Ok(eligible)

        }).await.map_err(|e|
            StateError::new(format!("Spawn findValue task error: {}", e))
        )?.and_then(|v| {
            if !v.is_empty() && v.needs_update() {
                let value = v.value().unwrap();
                let storage = self.storage.clone();
                task::spawn_local(async move {
                    storage.lock().unwrap().put_value(value, None)
                });
            }
            Ok(v.value())
        })
    }

    async fn do_find_value(
        value_id: Id,
        expected_seq: i32,
        option: LookupOption,
        dht4: Option<Arc<Mutex<DHT>>>,
        dht6: Option<Arc<Mutex<DHT>>>,
    ) -> Result<Option<Value>> {

        let mut futs = FuturesUnordered::new();
        futs.push(tokio::spawn(async move {
            if let Some(dht) = dht4 {
                DHT::find_value(dht, &value_id, expected_seq, option).await
            } else {
                Ok(None)
            }
        }));
        futs.push(tokio::spawn(async move {
            if let Some(dht) = dht6 {
                DHT::find_value(dht, &value_id, expected_seq, option).await
            } else {
                Ok(None)
            }
        }));

        let mut eligible = EligibleValue::new(value_id, expected_seq);
        for handle in futs.iter_mut() {
            let result = handle.await.map_err(|e| {
                StateError::new(format!("Spawn findValue task error: {}", e))
            })?;
            if let Some(value) = result? {
                eligible.update(value, true);
            }
        }
        Ok(eligible.value())
    }

    pub async fn find_peer(&self,
        peer_id: &Id,
        expected_sequence_number: i32,
        expected_count: usize,
        option: Option<LookupOption>
    ) -> Result<Vec<PeerInfo>> {
        if expected_sequence_number < -1 {
            return Err(ArgumentError::new(format!("Invalid expected sequence number: {}",
                expected_sequence_number
            )));
        }
        self.check_running()?;

        let option = option.unwrap_or(self.default_lookup_option());
        let storage = self.storage.clone();
        let dht4    = self.dht4.lock().unwrap().clone();
        let dht6    = self.dht6.lock().unwrap().clone();
        let peer_id = peer_id.clone();

        task::spawn_local(async move {
            let peers = storage.lock().unwrap().get_peers_with_expected_seq(
                &peer_id,
                expected_sequence_number,
                expected_count as i32
            )?;

            let mut eligible = EligiblePeers::new(
                peer_id.clone(),
                expected_sequence_number,
                expected_count
            );
            eligible.add(peers, false);
            if !eligible.is_empty() {
                if option == LookupOption::Local {
                    return Ok(eligible)
                }
                if option  != LookupOption::Conservative && expected_sequence_number >= 0 &&
                     eligible.reached_capacity() {
                    return Ok(eligible)
                }
            }

            let peers = Self::do_find_peer(
                peer_id,
                expected_sequence_number,
                expected_count,
                option,
                dht4,
                dht6
            ).await?;

            eligible.add(peers, true);
            Ok(eligible)

        }).await.map_err(|e|
            StateError::new(format!("Spawn findPeer task error: {}", e))
        )?.and_then(|v| {
            if !v.is_empty() && v.needs_update() {
                let storage = self.storage.clone();
                let peers = v.peers();
                task::spawn_local(async move {
                    storage.lock().unwrap().put_peers(peers)
                });
            }
            Ok(v.peers())
        })
    }

    async fn do_find_peer(
        peerid: Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption,
        dht4: Option<Arc<Mutex<DHT>>>,
        dht6: Option<Arc<Mutex<DHT>>>,
    ) -> Result<Vec<PeerInfo>> {

        let mut futs = FuturesUnordered::new();
        futs.push(task::spawn_local(async move {
            if let Some(dht) = dht4 {
                DHT::find_peer(dht, &peerid, expected_seq, expected_count, option).await
            } else {
                Ok(Vec::new())
            }
        }));
        futs.push(task::spawn_local(async move {
            if let Some(dht) = dht6 {
                DHT::find_peer(dht, &peerid, expected_seq, expected_count, option).await
            } else {
                Ok(Vec::new())
            }
        }));

        let mut eligible = EligiblePeers::new(
            peerid,
            expected_seq,
            expected_count
        );
        for handle in futs.iter_mut() {
            let result = handle.await.map_err(|e| {
                StateError::new(format!("Spawn findPeer task error: {}", e))
            })?;
            match result {
                Ok(peers) => eligible.add(peers, true),
                Err(e) => return Err(e),
            };
        }
        Ok({
            eligible.prune();
            eligible.peers()
        })
    }

    pub async fn store_value(&self,
        value: &Value,
        expected_sequence_number: i32,
        persistent: bool
    ) -> Result<()> {
        if !value.is_valid() {
            return Err(ArgumentError::new(format!("Invalid value")));
        }
        if expected_sequence_number < -1 {
            return Err(ArgumentError::new(format!("Invalid expected sequence number: {}",
                expected_sequence_number)));
        }
        self.check_running()?;

        let storage = self.storage.clone();
        let checked_value = value.clone();
        task::spawn_local(async move {
            Self::check_value(storage, &checked_value, expected_sequence_number)
        }).await.map_err(|e| {
            StateError::new(format!("Spawn check value task error: {}", e))
        })??;

        // store the value locally.
        let storage = self.storage.clone();
        let store_value = value.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().put_value(store_value, Some(persistent));
        }).await.map_err(|e| {
            StateError::new(format!("Spawn storage task error: {}", e))
        })?;

        Self::do_store_value(
            value,
            expected_sequence_number,
            self.dht4.lock().unwrap().clone(),
            self.dht6.lock().unwrap().clone()
        ).await?;

        let storage = self.storage.clone();
        let value_id = value.id();
        task::spawn_local(async move {
            storage.lock().unwrap().update_value_announced_time(&value_id)
        });
        Ok(())
    }

    fn check_value(
        storage: Arc<Mutex<Box<dyn DataStorage>>>,
        value: &Value,
        expected_seq: i32
    ) -> Result<()> {
        let result = storage.lock().unwrap().get_value(&value.id())?;
        let Some(existing) = result else {
            return Ok(());
        };

        let valueid = value.id();
        if existing.is_mutable() != value.is_mutable() {
            warn!("Rejecting value {} with mutability changed from {} to {}",
                valueid, existing.is_mutable(), value.is_mutable());
            return Err(ImmutableSubstitutionError::new());
        }
        if value.sequence_number() < existing.sequence_number() {
            warn!("Rejecting value {} with old sequence number {} < {}",
                valueid, value.sequence_number(), existing.sequence_number());
            return Err(SeqNotMonotonic::new());
        }
        if expected_seq >= 0 && existing.sequence_number() > expected_seq {
            warn!("Rejecting value {} with unexpected sequence number {} > {}",
                valueid, existing.sequence_number(), expected_seq);
            return Err(SeqNotExpected::new());
        }
        if existing.has_private_key() && !value.has_private_key() {
            warn!("Rejecting value {} with private key lost", valueid);
            return Err(NotOwnerError::new());
        }
        Ok(())
    }

    async fn do_store_value(
        value: &Value,
        expected_seq: i32,
        dht4: Option<Arc<Mutex<DHT>>>,
        dht6: Option<Arc<Mutex<DHT>>>,
    ) -> Result<()> {
        let mut futures = FuturesUnordered::new();
        futures.push({
            let value = value.clone();
            task::spawn_local(async move{
                if let Some(dht) = dht4 {
                    DHT::store_value(dht, value, expected_seq).await
                } else {
                    Ok(())
                }
            })}
        );
        futures.push({
            let value = value.clone();
            task::spawn_local(async move{
                if let Some(dht) = dht6 {
                    DHT::store_value(dht, value, expected_seq).await
                } else {
                    Ok(())
                }
            })}
        );
        for handle in futures.iter_mut() {
            handle.await.map_err(|e|
                NetworkError::new(format!("Spawn storeValue task error: {}", e))
            )??;
        }
        Ok(())
    }

    pub async fn announce_peer(&self,
        peer: PeerInfo,
        expected_sequence_number: i32,
        persistent: bool
    ) -> Result<()> {
        if !peer.is_valid() {
            return Err(ArgumentError::new(format!("Peer {} is invalid", peer.id())));
        }
        if expected_sequence_number < -1 {
            return Err(ArgumentError::new(format!("Invalid expected sequence number: {}",
                expected_sequence_number)));
        }
        self.check_running()?;

        // check the peer validity.
        let storage = self.storage.clone();
        let check_peer = peer.clone();
        task::spawn_local(async move {
            Self::check_peer(storage, &check_peer, expected_sequence_number)
        }).await.map_err(|e| {
            StateError::new(format!("Spawn check peer task error: {}", e))
        })??;

        // store the peer locally.
        let storage = self.storage.clone();
        let store_peer = peer.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().put_peer(store_peer, Some(persistent));
        }).await.map_err(|e| {
            StateError::new(format!("Spawn storage task error: {}", e))
        })?;

        // announce the peer to the network.
        Self::do_announce_peer(
            peer.clone(),
            expected_sequence_number,
            self.dht4.lock().unwrap().clone(),
            self.dht6.lock().unwrap().clone()
        ).await?;

        // update the peer announced time.
        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().update_peer_announced_time(
                peer.id(),
                peer.fingerprint()
            )
        });

        Ok(())
    }

    fn check_peer(
        storage: Arc<Mutex<Box<dyn DataStorage>>>,
        peer: &PeerInfo,
        expected_seq: i32
    ) -> Result<()> {
        let result = storage.lock().unwrap().get_peer(
            peer.id(), peer.fingerprint()
        )?;

        let Some(existing) = result else {
            return Ok(());
        };
        if peer.sequence_number() < existing.sequence_number() {
            warn!("Rejecting peer {} with old sequence number {} < {}",
                peer.id(), peer.sequence_number(), existing.sequence_number());
            return Err(SeqNotMonotonic::new());
        }
        if expected_seq >= 0 && existing.sequence_number() > expected_seq {
            warn!("Rejecting peer {} with unexpected sequence number {} > {}",
                peer.id(), existing.sequence_number(), expected_seq);
            return Err(SeqNotExpected::new());
        }
        if existing.has_private_key() && !peer.has_private_key() {
            warn!("Rejecting peer {} with private key lost", peer.id());
            return Err(NotOwnerError::new());
        }
        Ok(())
    }

    async fn do_announce_peer(
        peer: PeerInfo,
        expected_seq: i32,
        dht4: Option<Arc<Mutex<DHT>>>,
        dht6: Option<Arc<Mutex<DHT>>>,
    ) -> Result<()> {
        let mut futures = FuturesUnordered::new();
        futures.push({
            let peer = peer.clone();
            tokio::task::spawn_local(async move {
                if let Some(dht) = dht4 {
                    DHT::announce_peer(dht, peer, expected_seq).await;
                }
            })
        });
        futures.push({
            let peer = peer.clone();
            task::spawn_local(async move {
                if let Some(dht) = dht6 {
                    DHT::announce_peer(dht, peer, expected_seq).await;
                }
            })
        });
        for handle in futures.iter_mut() {
            handle.await.map_err(|e|
                StateError::new(format!("Spawn announcePeer task error: {}", e))
            )?;
        }
        Ok(())
    }

    pub async fn value(&self, value_id: Id) -> Result<Option<Value>> {
        self.check_running()?;

        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().get_value(&value_id)
        }).await.map_err(|e|
            DBError::new(format!("{}", e))
        )?
    }

    pub async fn remove_value(&self, value_id: Id) -> Result<()> {
        self.check_running()?;

        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().remove_value(&value_id)
        }).await.map_err(|e|
            DBError::new(format!("{}", e))
        )?
    }

    pub async fn peers(&self, peer_id: Id) -> Result<Vec<PeerInfo>> {
        self.check_running()?;

        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().get_peers(&peer_id)
        }).await.map_err(|e|
            DBError::new(format!("{}", e))
        )?
    }

    pub async fn remove_peers(&self, peer_id: Id) -> Result<()> {
        self.check_running()?;

        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().remove_peers(&peer_id)
        }).await.map_err(|e|
            DBError::new(format!("{}", e))
        )?
    }

    pub async fn peer(&self, peer_id: Id, finger_print: u64) -> Result<Option<PeerInfo>> {
        self.check_running()?;

        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().get_peer(&peer_id, finger_print)
        }).await.map_err(|e|
            DBError::new(format!("{}", e))
        )?
    }

    pub async fn remove_peer(&self, peer_id: Id, finger_print: u64) -> Result<()> {
        self.check_running()?;

        let storage = self.storage.clone();
        task::spawn_local(async move {
            storage.lock().unwrap().remove_peer(&peer_id, finger_print)
        }).await.map_err(|e|
            DBError::new(format!("{}", e))
        )?
    }

    pub fn sign(&self, data: &[u8], signature:&mut [u8]) -> Result<usize> {
        Identity::sign(self, data, signature)
    }

    pub fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        Identity::sign_into(self, data)
    }

    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        Identity::verify(self, data, signature)
    }

    pub fn encrypt(&self, receiver: &Id, data: &[u8], cipher: &mut [u8]) -> Result<usize> {
        Identity::encrypt(self, receiver, data, cipher)
    }

    pub fn encrypt_into(&self, receiver: &Id, data: &[u8]) -> Result<Vec<u8>> {
        Identity::encrypt_into(self, receiver, data)
    }

    pub fn decrypt(&self, sender: &Id, data: &[u8], plain: &mut [u8]) -> Result<usize> {
        Identity::decrypt(self, sender, data, plain)
    }

    pub fn decrypt_into(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>> {
        Identity::decrypt_into(self, sender, data)
    }
}

fn get_keypair(path: &str) -> Result<signature::KeyPair> {
    create_dirs(path).map_err(|e| {
        return StateError::new(format!("Checking persistence error: {}", e));
    }).ok().unwrap();

    let keypath = path.to_string() + "key";
    let keypair;

    match fs::metadata(&keypath) {
        Ok(metadata) => {
            // Loading key from persistence.
            if metadata.is_dir() {
                return Err(StateError::new(format!("Bad file path {} for key storage.", keypath)));
            };
            keypair = load_key(&keypath)
                .map_err(|e| StateError::new(format!("Error loading key: {}", e)))?
        },
        Err(_) => {
            // otherwise, generate a fresh keypair
            keypair = signature::KeyPair::random();
            store_key(&keypath, &keypair)
                .map_err(|e|return e)
                .ok()
                .unwrap();
        }
    };

    Ok(keypair)
}

impl Identity for Node {
    fn id(&self) -> &Id {
        Identity::id(&self.identity)
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        Identity::sign(&self.identity, data, signature)
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        Identity::verify(&self.identity, data, signature)
    }

    fn encrypt(&self, receiver: &Id, data: &[u8], cipher: &mut [u8]) -> Result<usize> {
        Identity::encrypt(&self.identity, receiver, data, cipher)
    }

    fn decrypt(&self, sender: &Id, data: &[u8], plain: &mut [u8]) -> Result<usize> {
        Identity::decrypt(&self.identity, sender, data, plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        Identity::create_crypto_context(&self.identity, id)
    }
}

use std::str;
fn load_key(path: &str) -> Result<signature::KeyPair> {
    let mut fp = match File::open(path) {
        Ok(v) => v,
        Err(e) => return Err(IOError::new(
            format!("Openning key file error: {}", e))),
    };

    let mut buf = Vec::new();
    if let Err(e) = fp.read_to_end(&mut buf) {
        return Err(IOError::new(format!("Reading key error: {}", e)));
    };

    let sk: signature::PrivateKey = str::from_utf8(&buf).map_err(|e| {
        return StateError::new(format!("Key file is not UTF-8: {}", e));
    })?.try_into().map_err(|e| {
        return StateError::new(format!("Key file is not a valid key: {}", e));
    })?;

    Ok(signature::KeyPair::from(&sk))
}

fn store_key(path: &str, keypair: &signature::KeyPair) -> Result<()> {
    let mut fp = match File::create(path) {
        Ok(v) => v,
        Err(e) => return Err(IOError::new(
            format!("Creating key file error: {}", e))),
    };

    let result = fp.write_all(keypair.private_key().to_string().as_bytes());
    if let Err(e) = result {
        return Err(IOError::new(format!("Writing key error: {}", e)));
    }
    Ok(())
}

fn store_nodeid(path: &str, id: &Id) -> Result<()> {
    let mut fp = match File::create(path) {
        Ok(v) => v,
        Err(e) => return Err(IOError::new(
            format!("Creating Id file error: {}", e))),
    };

    let result = fp.write_all(id.to_base58().as_bytes());
    if let Err(e) = result {
        return Err(IOError::new(format!("Writing ID error: {}", e)));
    };
    Ok(())
}
