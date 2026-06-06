use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration
};
use log::{warn, info};
use tokio::{
    task::JoinHandle,
    sync::mpsc
};

use crate::{
    CryptoContext, CryptoIdentity,
    Id, Identity, JointResult, Network,
    NodeInfo, PeerInfo, Value,
    core::{logger,version},
    Result,
    errors::{ ArgumentError, IOError, StateError},
    signature
};
use crate::dht::{
    dht::{self, DHT},
    NodeConfig,
    LookupOption,
    node_status::NodeStatus,
    eligible_value::EligibleValue,
    eligible_peers::EligiblePeers,
    cached_identity::CachedIdentity,
    token_manager::TokenManager,
    timer_client::TimerClient,
    timer_queue::{TimerQueue, Command},
    storage::{
        data_storage::{self, DataStorage},
        sqlite_storage::SqliteStorage,
    },
    errors::{
        SeqNotExpected,
        SeqNotMonotonic,
        NotOwnerError,
        ImmutableSubstitutionError
    }
};

const MAX_PEER_AGE  : Duration = Duration::from_millis(120 * 60 * 1000); // 2 hours in milliseconds
const MAX_VALUE_AGE : Duration = Duration::from_millis(120 * 60 * 1000); // 2 hours in milliseconds

const RE_ANNOUNCE_INTERVAL : Duration = Duration::from_millis(5 * 60 * 1000);        // 5 minutes in milliseconds
const STORAGE_EXPIRE_INTERVAL : Duration = Duration::from_millis(10 * 60 * 1000);    // 10 minutes in milliseconds

pub struct Node {
    cfg         : Box<dyn NodeConfig>,
    identity    : CachedIdentity,

    lookup_option  : Mutex<LookupOption>,
    status      : Mutex<NodeStatus>,
    data_dir    : String,

    dht4        : Mutex<Option<Arc<Mutex<DHT>>>>,
    dht6        : Mutex<Option<Arc<Mutex<DHT>>>>,

    timer_client: Mutex<Option<Arc<TimerClient>>>,
    timer_task  : Mutex<Option<JoinHandle<()>>>,

    storage     : Arc<Mutex<Box<dyn DataStorage>>>,
    tokenman    : Arc<TokenManager>,
}

impl Node {
    pub fn new(cfg: Box<dyn NodeConfig>) -> Result<Arc<Self>> {
        Self::check_config(cfg.as_ref())?;

        // Setup logger before any log is generated.
        logger::setup(
            cfg.as_ref().log_level(),
            cfg.as_ref().log_file()
        );
        logger::enable_console_output();

        #[cfg(feature = "devp")]
        info!("DHT node running in development mode!!!");

        let data_dir = cfg.data_dir().into();
        let database_uri = {
            let mut path = PathBuf::new();
            path.push(cfg.data_dir());
            path.push(data_storage::database_name(cfg.database_uri()));
            path
        };
        let storage_db = {
            let mut storage = SqliteStorage::new();
            storage.open(database_uri.to_str().unwrap())?;
            storage.initialize(MAX_VALUE_AGE, MAX_PEER_AGE)?;
            storage
        };

        let identity = CachedIdentity::new({
            let kp = signature::KeyPair::from(cfg.private_key());
            CryptoIdentity::from_keypair(kp)
        });

        // Cache the node id to a file for quick access in the future.
        let bs58 = identity.id().to_base58();
        File::create({
            let mut path = PathBuf::new();
            path.push(cfg.data_dir());
            path.push("id");
            path
        }).map_err(|e| IOError::new(
                format!("Creating node id cache file error: {e}")))?
            .write_all(bs58.as_bytes()).map_err(|e| IOError::new(
                format!("Writing node id cache file error: {e}")))?;

        info!("Current Node id: {}", identity.id());

        Ok(Arc::new(Self {
            cfg,
            identity,
            data_dir,
            lookup_option   : Mutex::new(LookupOption::Conservative),
            status          : Mutex::new(NodeStatus::Stopped),
            dht4            : Mutex::new(None),
            dht6            : Mutex::new(None),
            timer_client    : Mutex::new(None),
            timer_task      : Mutex::new(None),
            storage         : Arc::new(Mutex::new(Box::new(storage_db))),
            tokenman        : Arc::new(TokenManager::new())
        }))
    }

    fn check_config(cfg: &dyn NodeConfig) -> Result<()> {
        if cfg.host4().is_none() && cfg.host6().is_none() {
            return Err(ArgumentError::new(
                "At least one host/address must be specified"));
        }
        //if cfg.bootstrap_nodes().is_empty() {
        //    return Err(ArgumentError::new(
        //        "At least one bootstrap node must be specified"));
        //}

        let data_dir = cfg.data_dir();
        if data_dir.is_empty() {
            return Err(ArgumentError::new("Data directory cannot be empty"));
        }
        let path = Path::new(data_dir);
        if path.exists() {
            if !path.is_dir() {
                return Err(ArgumentError::new(
                    format!("Data path {} is not a directory", data_dir)))
            }
        } else {
            fs::create_dir_all(path).map_err(|e| {
                ArgumentError::new(
                    format!("Data path {} can not be created: {}", data_dir, e))
            })?;
        };

        let database_uri = cfg.database_uri();
        if database_uri.is_empty() {
            return Err(ArgumentError::new("Database URI cannot be empty"));
        }
        if database_uri.contains("/") {
            return Err(ArgumentError::new(
                "Database URI cannot contain path separator '/'"));
        }
        if !data_storage::supports(database_uri) {
            return Err(ArgumentError::new(
                format!("Unsupported database URI: {}", database_uri)));
        }
        Ok(())
    }

    #[inline(always)]
    fn dht4(&self) -> Option<Arc<Mutex<DHT>>> {
        self.dht4.lock().unwrap().clone()
    }

    #[inline(always)]
    fn dht6(&self) -> Option<Arc<Mutex<DHT>>> {
        self.dht6.lock().unwrap().clone()
    }

    #[inline(always)]
    fn option(&self, option: Option<LookupOption>) -> LookupOption {
        option.unwrap_or_else(|| self.default_lookup_option())
    }

    #[inline(always)]
    fn check_running(&self) -> Result<()> {
        match self.is_running() {
            true => Ok(()),
            false => Err(StateError::new("Node is not running".to_string()))
        }
    }

    #[inline(always)]
    async fn start_timer_runtime(&self) {
        let (tx, rx) = mpsc::channel::<Command>(64);
        let timer_queue = TimerQueue::new(rx);
        let timer_client = Arc::new(TimerClient::new(tx));

        let timer_task = tokio::spawn(async move {
            timer_queue.run().await;
        });

        *self.timer_task.lock().unwrap() = Some(timer_task);
        *self.timer_client.lock().unwrap() = Some(timer_client);
    }

    #[inline(always)]
    async fn stop_timer_runtime(&self) {
        let timer_client = self.timer_client.lock().unwrap().take();
        let timer_task = self.timer_task.lock().unwrap().take();

        if let Some(timer_client) = timer_client {
            let _ = timer_client.stop_all().await;
        }
        if let Some(timer_task) = timer_task {
            let _ = timer_task.await;
        }
    }

    #[inline(always)]
    fn timer_client(&self) -> Arc<TimerClient> {
        self.timer_client.lock().unwrap()
            .as_ref().expect("Timer client is not initalized")
            .clone()
    }

    pub async fn start(&self) -> Result<()> {
        if *self.status.lock().unwrap() != NodeStatus::Stopped {
            return Ok(());
        }
        *self.status.lock().unwrap() = NodeStatus::Initializing;

        self.start_timer_runtime().await;
        if let Err(err) = self.setup_periodic_tasks().await {
            self.stop_timer_runtime().await;
            *self.status.lock().unwrap() = NodeStatus::Stopped;
            return Err(err);
        }

        let port = self.cfg.port();
        let mut builder = dht::Builder::new();
        builder.with_timer_client(self.timer_client())
            .with_identity(self.identity.identity())
            .with_storage(self.storage.clone())
            .with_tokenman(self.tokenman.clone())
            .with_bootstrap_nodes(self.cfg.bootstrap_nodes())
            .with_datadir(self.data_dir.as_str());

        if let Some(host4) = self.cfg.host4() {
            let rc = builder.build_dht4(host4, port);
            if let Err(err) = rc {
                self.stop_timer_runtime().await;
                *self.status.lock().unwrap() = NodeStatus::Stopped;
                return Err(err);
            }

            let mut dht4 = rc.unwrap();
            dht4.set_connection_status_listener();

            if let Err(err) = dht4.start().await {
                self.stop_timer_runtime().await;
                *self.status.lock().unwrap() = NodeStatus::Stopped;
                return Err(err);
            }

            *self.dht4.lock().unwrap() = Some({
                let dht = Arc::new(Mutex::new(dht4));
                dht.lock().unwrap().weak_cloned = Arc::downgrade(&dht);
                dht
            });
        }

        if let Some(host6) = self.cfg.host6() {
            let rc = builder.build_dht6(host6, port);
            if let Err(err) = rc {
                if let Some(dht4) = self.dht4.lock().unwrap().take() {
                    dht4.lock().unwrap().stop().await;
                }
                self.stop_timer_runtime().await;
                *self.status.lock().unwrap() = NodeStatus::Stopped;
                return Err(err);
            }

            let mut dht6 = rc.unwrap();
            dht6.set_connection_status_listener();

            if let Err(err) = dht6.start().await {
                if let Some(dht4) = self.dht4.lock().unwrap().take() {
                    dht4.lock().unwrap().stop().await;
                }
                self.stop_timer_runtime().await;
                *self.status.lock().unwrap() = NodeStatus::Stopped;
                return Err(err);
            }

            *self.dht6.lock().unwrap() = Some({
                let dht = Arc::new(Mutex::new(dht6));
                dht.lock().unwrap().weak_cloned = Arc::downgrade(&dht);
                dht
            });
        }

        *self.status.lock().unwrap() = NodeStatus::Running;
        info!("Kademlia node started.");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if *self.status.lock().unwrap() == NodeStatus::Stopped {
            return Ok(());
        }
        *self.status.lock().unwrap() = NodeStatus::Stopped;

        let dht4 = self.dht4.lock().unwrap().take();
        let dht6 = self.dht6.lock().unwrap().take();

        if let Some(dht) = dht4 {
            dht.lock().unwrap().stop().await;
        }
        if let Some(dht) = dht6 {
            dht.lock().unwrap().stop().await;
        }

        self.stop_timer_runtime().await;
        self.storage.lock().unwrap().close()?;

        info!("Kademlia node stopped.");
        logger::teardown();

        Ok(())
    }

    async fn setup_periodic_tasks(&self) -> Result<()> {
        let timer_client = self.timer_client.lock().unwrap()
            .as_ref().expect("Timer client is not initalized")
            .clone();

        let storage = self.storage.clone();
        let _ = timer_client.add_timer(
            Duration::from_millis(30*1000),
            Some(STORAGE_EXPIRE_INTERVAL),
            move || {
                _ = storage.lock().unwrap().purge();
            }
        ).await?;

        let storage = self.storage.clone();
        let _ = timer_client.add_timer(
            Duration::from_millis(5*60*1000),
            Some(RE_ANNOUNCE_INTERVAL),
            move || {
                // TODO:
                let _ = storage.clone();
                unimplemented!();
            }
        ).await?;

        let tokenman = self.tokenman.clone();
        let _ = timer_client.add_timer(
            TokenManager::TOKEN_TIMEOUT,
            Some(TokenManager::TOKEN_TIMEOUT),
            move || {
                tokenman.update_token_timestamp();
            }
        ).await?;
        Ok(())
    }

    pub fn id(&self) -> &Id {
        self.identity.id()
    }

    pub fn node_info(&self) -> JointResult<NodeInfo> {
        let mut result = JointResult::new();
        if let Some(dht) = self.dht4() {
            let ni = Arc::unwrap_or_clone(dht.lock().unwrap().ni());
            result.set_value(Network::IPv4, ni);
        }
        if let Some(dht) = self.dht6() {
            let ni = Arc::unwrap_or_clone(dht.lock().unwrap().ni());
            result.set_value(Network::IPv6, ni);
        }
        result
    }

    pub fn version(&self) -> String {
        version::format_version(version::ver())
    }

    pub fn set_default_lookup_option(&self, option: LookupOption) {
        *self.lookup_option.lock().unwrap() = option;
    }

    pub fn default_lookup_option(&self) -> LookupOption {
        self.lookup_option.lock().unwrap().clone()
    }

    pub fn is_running(&self) -> bool {
        *self.status.lock().unwrap() == NodeStatus::Running
    }

    pub async fn bootstrap_one(&self,  node: &NodeInfo) -> Result<()> {
        self.bootstrap(&[node.clone()]).await
    }

    pub async fn bootstrap(&self, nodes: &[NodeInfo]) -> Result<()> {
        if nodes.is_empty() {
            return Err(ArgumentError::new("Bootstrap nodes cannot be empty"));
        }
        self.check_running()?;

        let dht4    = self.dht4();
        let dht6    = self.dht6();
        let nodes4  = nodes.to_vec();
        let nodes6  = nodes.to_vec();

        tokio::join!(
            async move {
                if let Some(dht) = dht4 {
                    DHT::bootstrap(dht, nodes4).await;
                }
            },
            async move {
                if let Some(dht) = dht6 {
                    DHT::bootstrap(dht, nodes6).await;
                }
            }
        );
        Ok(())
    }

    pub async fn find_node(
        &self,
        target: &Id,
        lookup_option: Option<LookupOption>
    ) -> Result<JointResult<NodeInfo>> {
        self.check_running()?;

        let target  = target.clone();
        let option  = self.option(lookup_option);
        let dht4    = self.dht4();
        let dht6    = self.dht6();

        let rc = tokio::select!{
            v = async move {
                if let Some(dht) = dht4 {
                    dht.lock().unwrap().find_node(&target, option).await
                } else {
                    Ok(None)
                }
            }, if dht4.is_some() => (Network::IPv4, v),
            v = async move {
                if let Some(dht) = dht6 {
                    dht.lock().unwrap().find_node(&target, option).await
                } else {
                    Ok(None)
                }
            }, if dht6.is_some() => (Network::IPv6, v),
        };

        Ok({
            let mut jres = JointResult::<NodeInfo>::new();
            if let Some(ni) = rc.1? {
                jres.set_value(rc.0, ni);
            }
            jres
        })
    }

    pub async fn find_value(&self,
        value_id: &Id,
        expected_seq: i32,
        lookup_option: Option<LookupOption>
    ) -> Result<Option<Value>> {
        if expected_seq < -1 {
            return Err(ArgumentError::new(
                format!("Invalid expected sequence number: {expected_seq}")));
        }
        self.check_running()?;

        let target  = value_id.clone();
        let option  = self.option(lookup_option);
        let dht4    = self.dht4();
        let dht6    = self.dht6();

        let mut eligible = EligibleValue::new(
            target, expected_seq
        );

        let value = crate::locked!(self.storage).get_value(&target)?;
        if let Some(v) = value {
            let is_mutable = v.is_mutable();
            eligible.update(v, false);

            if !is_mutable {
                return Ok(eligible.value());
            }
            if option != LookupOption::Conservative && !eligible.is_empty() {
                return Ok(eligible.value());
            }
        }

        let rc = tokio::select!(
            v = async move {
                if let Some(dht) = dht4 {
                    crate::locked!(dht).find_value(
                        &target, expected_seq, option).await
                } else {
                    Ok(None)
                }
            }, if dht4.is_some() => v,
            v = async move {
                if let Some(dht) = dht6 {
                    crate::locked!(dht).find_value(
                        &target, expected_seq, option).await
                } else {
                    Ok(None)
                }
            }, if dht6.is_some() => v
        );

        if let Some(value) = rc? {
            eligible.update(value, true);
        }

        if !eligible.is_empty() && eligible.is_latest() {
            let _ = crate::locked!(self.storage).put_value(
                eligible.value().unwrap(), false
            );
        }

        Ok(eligible.value())
    }

    pub async fn find_peer(&self,
        peer_id: &Id,
        expected_seq: i32,
        expected_count: usize,
        lookup_option: Option<LookupOption>
    ) -> Result<Vec<PeerInfo>> {
        if expected_seq < -1 {
            return Err(ArgumentError::new(
                format!("Invalid expected sequence number: {expected_seq}")));
        }
        self.check_running()?;

        let target  = peer_id.clone();
        let option  = self.option(lookup_option);
        let dht4    = self.dht4();
        let dht6    = self.dht6();

        let mut eligible = EligiblePeers::new(
            target, expected_seq, expected_count
        );

        let peers = crate::locked!(self.storage).get_peers_with_expected_seq(
                &target, expected_seq, expected_count as i32)?;
        eligible.add(peers, false);
        eligible.prune();

        if !eligible.is_empty() {
            if option == LookupOption::Local {
                return Ok(eligible.peers())
            }
            if option  != LookupOption::Conservative &&
                expected_seq >= 0 && eligible.reached_capacity() {
                return Ok(eligible.peers())
            }
        }

        let rc = tokio::select!(
            v = async move {
                if let Some(dht) = dht4 {
                    crate::locked!(dht).find_peer(
                        &target, expected_seq, expected_count, option).await
                } else {
                    Ok(Vec::new())
                }
            }, if dht4.is_some() => v,
            v = async move {
                if let Some(dht) = dht6 {
                    crate::locked!(dht).find_peer(
                        &target, expected_seq, expected_count, option).await
                } else {
                    Ok(Vec::new())
                }
            }, if dht6.is_some() => v
        );

        eligible.add(rc?, true);
        eligible.prune();

        if !eligible.is_empty() && eligible.is_latest() {
            let _ = self.storage.lock().unwrap().put_peers(eligible.peers());
        }
        Ok(eligible.peers())
    }

    pub async fn store_value(&self,
        value: &Value,
        expected_seq: i32,
        persistent: bool
    ) -> Result<()> {
        if !value.is_valid() {
            return Err(ArgumentError::new("The value is verified to be invalid"));
        }
        if expected_seq < -1 {
            return Err(ArgumentError::new(
                format!("Invalid expected sequence number: {expected_seq}")));
        }
        self.check_running()?;

        let result = self.storage.lock().unwrap().get_value(&value.id())?;
        if let Some(ref existing) = result {
            check_value(existing, value, expected_seq)?;
        };

        // store the value in local node.
        let _ = self.storage.lock().unwrap().put_value(
            value.clone(), persistent
        )?;

        // store the value to the network.
        let value4  = value.clone();
        let value6  = value.clone();
        let dht4    = self.dht4();
        let dht6    = self.dht6();

        let (rc4, rc6) = tokio::join!(
            async move {
                if let Some(dht) = dht4 {
                    dht.lock().unwrap().store_value(value4, expected_seq).await
                } else {
                    Ok(())
                }
            },
            async move {
                if let Some(dht) = dht6 {
                    dht.lock().unwrap().store_value(value6, expected_seq).await
                } else {
                    Ok(())
                }
            }
        );
        for rc in [rc4, rc6] {
            let _ = rc?;
        }

        let value_id = value.id();
        let _ = crate::locked!(self.storage)
                .update_value_announced_time(&value_id);
        Ok(())
    }

    pub async fn announce_peer(&self,
        peer: &PeerInfo,
        expected_seq: i32,
        persistent: bool
    ) -> Result<()> {
        if !peer.is_valid() {
            return Err(ArgumentError::new("The peer is verified to be invalid."));
        }
        if expected_seq < -1 {
            return Err(ArgumentError::new(
                format!("Invalid expected sequence number: {expected_seq}")));
        }
        self.check_running()?;

        let result = crate::locked!(self.storage).get_peer(
            peer.id(), peer.fingerprint()
        )?;

        // check the peer validity.
        if let Some(ref existing) = result {
            check_peer(existing, peer, expected_seq)?;
        }

        // store the new peer locally.
        let _ = crate::locked!(self.storage).put_peer(
            peer.clone(), persistent
        );

        // announce the peer to the network.
        let peer4   = peer.clone();
        let peer6   = peer.clone();
        let dht4    = self.dht4();
        let dht6    = self.dht6();

        let (rc4, rc6) = tokio::join!(
            async move {
                if let Some(dht) = dht4 {
                    dht.lock().unwrap()
                        .announce_peer(peer4, expected_seq).await
                } else {
                    Ok(())
                }
            },
            async move {
                if let Some(dht) = dht6 {
                    dht.lock().unwrap()
                        .announce_peer(peer6, expected_seq).await
                } else {
                    Ok(())
                }
            }
        );
        for rc in [rc4, rc6] {
            let _ = rc?;
        }

        // update the peer announced time.
        _ = crate::locked!(self.storage)
                .update_peer_announced_time(peer.id(), peer.fingerprint());
        Ok(())
    }

    pub fn value(&self, value_id: Id) -> Result<Option<Value>> {
        self.check_running()?;
        crate::locked!(self.storage).get_value(&value_id)
    }

    pub fn remove_value(&self, value_id: Id) -> Result<()> {
        self.check_running()?;
        crate::locked!(self.storage).remove_value(&value_id)
    }

    pub async fn peers(&self, peer_id: Id) -> Result<Vec<PeerInfo>> {
        self.check_running()?;
        crate::locked!(self.storage).get_peers(&peer_id)
    }

    pub async fn remove_peers(&self, peer_id: Id) -> Result<()> {
        self.check_running()?;
        crate::locked!(self.storage).remove_peers(&peer_id)
    }

    pub async fn peer(&self, peer_id: Id, finger_print: u64) -> Result<Option<PeerInfo>> {
        self.check_running()?;
        crate::locked!(self.storage).get_peer(&peer_id, finger_print)
    }

    pub async fn remove_peer(&self, peer_id: Id, finger_print: u64) -> Result<()> {
        self.check_running()?;
        crate::locked!(self.storage).remove_peer(&peer_id, finger_print)
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

impl Identity for Node {
    fn id(&self) -> &Id {
        self.identity.id()
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        self.identity.sign(data, signature)
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        self.identity.verify(data, signature)
    }

    fn encrypt(&self, receiver: &Id, data: &[u8], cipher: &mut [u8]) -> Result<usize> {
        self.identity.encrypt(receiver, data, cipher)
    }

    fn decrypt(&self, sender: &Id, data: &[u8], plain: &mut [u8]) -> Result<usize> {
        self.identity.decrypt(sender, data, plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        self.identity.create_crypto_context(id)
    }
}

fn check_value(old: &Value, new: &Value, expected_seq: i32) -> Result<()> {
    let valueid = new.id();
    if old.is_mutable() != new.is_mutable() {
        warn!("Rejecting value {} with mutability changed from {} to {}",
            valueid, old.is_mutable(), new.is_mutable());
        return Err(ImmutableSubstitutionError::new());
    }
    if new.sequence_number() < old.sequence_number() {
        warn!("Rejecting value {} with old sequence number {} < {}",
            valueid, new.sequence_number(), old.sequence_number());
        return Err(SeqNotMonotonic::new());
    }
    if expected_seq >= 0 && old.sequence_number() > expected_seq {
        warn!("Rejecting value {} with unexpected sequence number {} > {}",
            valueid, old.sequence_number(), expected_seq);
        return Err(SeqNotExpected::new());
    }
    if old.has_private_key() && !new.has_private_key() {
        warn!("Rejecting value {} with private key lost", valueid);
        return Err(NotOwnerError::new());
    }
    Ok(())
}

fn check_peer(old: &PeerInfo, new: &PeerInfo, expected_seq: i32) -> Result<()> {
    if new.sequence_number() < old.sequence_number() {
        warn!("Rejecting peer {} with old sequence number {} < {}",
            new.id(), new.sequence_number(), old.sequence_number());
        return Err(SeqNotMonotonic::new());
    }
    if expected_seq >= 0 && old.sequence_number() > expected_seq {
        warn!("Rejecting peer {} with unexpected sequence number {} > {}",
            new.id(), old.sequence_number(), expected_seq);
        return Err(SeqNotExpected::new());
    }
    if old.has_private_key() && !new.has_private_key() {
        warn!("Rejecting peer {} with private key lost", new.id());
        return Err(NotOwnerError::new());
    }
    Ok(())
}
