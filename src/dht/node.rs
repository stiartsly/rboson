use std::{
    fs, fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime}
};
use futures::{
    future::LocalBoxFuture,
    stream::FuturesUnordered,
    FutureExt,
    StreamExt
};
use log::{warn, info, debug};

use crate::{
    Id,
    Network,
    CryptoContext, CryptoIdentity, Identity,
    NodeInfo, PeerInfo, Value,
    JointResult,
    core::{logger,version},
    errors::{Result, ArgumentError, IOError, StateError},
    signature
};
use crate::dht::{
    NodeConfig,
    LookupOption,
    eligible_value::EligibleValue,
    eligible_peers::EligiblePeers,
    cached_identity::CachedIdentity,
    token_manager::TokenManager,
    handler::AsyncHandler,
    promise::Promise,
    connection_status::ConnectionStatus,
    connection_status_listener::ConnectionStatusListener,
    storage::{
        data_storage::{self, DataStorage},
        sqlite_storage::SqliteStorage,
    },
    errors::{
        SeqNotExpected,
        SeqNotMonotonic,
        NotOwnerError,
        ImmutableSubstitutionError
    },
    timer_verticle,
    dht_verticle::{self, VerticleClient, VerticleOptions},
};

const MAX_PEER_AGE  : Duration = Duration::from_millis(120 * 60 * 1000); // 2 hours in milliseconds
const MAX_VALUE_AGE : Duration = Duration::from_millis(120 * 60 * 1000); // 2 hours in milliseconds

const RE_ANNOUNCE_INTERVAL      : u64 = 5 * 60 * 1000;      // 5 minutes in milliseconds
const STORAGE_EXPIRE_INTERVAL   : u64 = 10 * 60 * 1000;     // 10 minutes in milliseconds

pub struct Node {
    cfg             : Box<dyn NodeConfig>,
    identity        : CachedIdentity,

    lookup_option   : Mutex<LookupOption>,

    dht4            : Mutex<Option<Arc<VerticleClient>>>,
    dht6            : Mutex<Option<Arc<VerticleClient>>>,

    data_dir        : PathBuf,
    database_uri    : PathBuf,

    running         : Mutex<bool>,
    listeners       : Arc<Mutex<Vec<Box<dyn ConnectionStatusListener>>>>,

    timer_verticle  : Mutex<Option<Arc<timer_verticle::VerticleClient>>>,
    //timer_task      : Mutex<Option<JoinHandle<()>>>,

    storage         : Arc<Mutex<dyn DataStorage>>,
    token_man       : Arc<TokenManager>,
    weak            : Weak<Self>,
}

impl Node {
    pub fn new(cfg: Box<dyn NodeConfig>) -> Result<Arc<Self>> {
        Self::check_config(&cfg)?;

        // Setup logger before any log is generated.
        logger::setup(
            cfg.as_ref().log_level(),
            cfg.as_ref().log_file()
        );
        logger::enable_console_output();

        #[cfg(feature = "devp")]
        info!("DHT node running in development mode!!!");

        let data_dir = {
            let mut path = PathBuf::new();
            path.push(cfg.data_dir());
            path
        };
        let database_uri = {
            let mut path = PathBuf::from(&data_dir);
            path.push(data_storage::database_name(cfg.database_uri()));
            path
        };

        let identity = CachedIdentity::new({
            let kp = signature::KeyPair::from(cfg.private_key());
            CryptoIdentity::from_keypair(kp)
        });

        // Cache the node id to a file for quick access in the future.
        let bs58 = identity.id().to_base58();
        let path = data_dir.clone().join("id");
        File::create(&path).map_err(|e| IOError::new(
                format!("Creating node id cache file error: {e}")))?
            .write_all(bs58.as_bytes()).map_err(|e| IOError::new(
                format!("Writing node id cache file error: {e}")))?;

        info!("The Kad node ID: {}", identity.id());

        Ok(Arc::new_cyclic(|weak| Self {
            cfg,
            identity,
            data_dir,
            database_uri,
            lookup_option   : Mutex::new(LookupOption::Conservative),
            dht4            : Mutex::new(None),
            dht6            : Mutex::new(None),

            running         : Mutex::new(false),
            listeners       : Arc::new(Mutex::new(Vec::new())),

            timer_verticle  : Mutex::new(None),

            storage         : Arc::new(Mutex::new(SqliteStorage::new())),
            token_man       : Arc::new(TokenManager::new()),
            weak            : weak.clone(),
        }))
    }

    fn check_config(cfg: &Box<dyn NodeConfig>) -> Result<()> {
        if cfg.host4().is_none() && cfg.host6().is_none() {
            return Err(ArgumentError::new(
                "At least one host/address must be specified"));
        }

        //if cfg.bootstrap_nodes().is_empty() {
        //    return Err(ArgumentError::new(
        //        "At least one bootstrap node must be specified"));
        //}

        if cfg.data_dir().is_empty() {
            return Err(ArgumentError::new("Data directory cannot be empty"));
        }

        let data_dir = cfg.data_dir();
        let path = Path::new(data_dir);
        if path.exists() {
            if !path.is_dir() {
                return Err(ArgumentError::new(format!(
                    "Data path {} is not a directory", data_dir)))
            }
        } else {
            fs::create_dir_all(path).map_err(|e| {
                ArgumentError::new(format!(
                    "Data path {} can not be created: {}", data_dir, e))
            })?;
        };

        let database_uri = cfg.database_uri();
        if database_uri.is_empty() {
            return Err(ArgumentError::new("Database URI cannot be empty"));
        }
        if database_uri.contains("/") {
            return Err(ArgumentError::new("Database URI cannot contain path separator '/'"));
        }
        if !data_storage::supports(database_uri) {
            return Err(ArgumentError::new(format!("Unsupported database URI: {}", database_uri)));
        }
        Ok(())
    }

    #[inline]
    fn option(&self, option: Option<LookupOption>) -> LookupOption {
        option.unwrap_or_else(|| self.default_lookup_option())
    }

    #[inline]
    fn check_running(&self) -> Result<()> {
        match self.is_running() {
            true => Ok(()),
            false => Err(StateError::new("kadNode is not running"))
        }
    }

    #[inline]
    fn timer_verticle(&self) -> Arc<timer_verticle::VerticleClient> {
        self.timer_verticle.lock().unwrap()
            .as_ref().expect("Timer verticle is not initialized")
            .clone()
    }

    async fn persistent_announce(self: Arc<Self>) {
        info!("Re-announce the persistent values and peers...");

        let storage = self.storage.clone();

        // Re-announce values
        let before = crate::as_ms!(SystemTime::now()) as u64
            - MAX_VALUE_AGE.as_millis() as u64
            + RE_ANNOUNCE_INTERVAL * 2;

        let values = match storage.lock().unwrap()
                .get_values_announced_before(true, before) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to re-announce the values: {}", e);
                Vec::new()
            }
        };

        let mut futures = FuturesUnordered::<LocalBoxFuture<'static, ()>>::new();

        for value in values {
            info!("Re-announce the value: {}", value.id());

            let node = self.clone();
            let task: LocalBoxFuture<'static, ()> = async move {
                let value_id = value.id();
                match node.store_value(&value, value.sequence_number(), true).await {
                    Ok(_) => info!("Re-announce the value {} success", value_id),
                    Err(e) => warn!("Re-announce the value {} failed: {}", value_id, e),
                }
            }.boxed_local();
            futures.push(task);
        }

        // Re-announce peers
        let before_peer = crate::as_ms!(SystemTime::now()) as u64
            - MAX_PEER_AGE.as_millis() as u64
            + RE_ANNOUNCE_INTERVAL * 2;

        let peers = match storage.lock().unwrap().get_peers_announced_before(true, before_peer) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to re-announce the peers: {}", e);
                Vec::new()
            }
        };

        for peer in peers {
            info!("Re-announce the peer: {}", peer.id());

            let node = self.clone();
            let task: LocalBoxFuture<'static, ()> = async move {
                let peer_id = peer.id().clone();
                match node.announce_peer(&peer, -1, true).await {
                    Ok(_) => info!("Re-announce the peer {} success", peer_id),
                    Err(e) => warn!("Re-announce the peer {} failed: {}", peer_id, e),
                }
            }.boxed_local();
            futures.push(task);
        }

        while futures.next().await.is_some() {
        }
    }

    async fn setup_periodic_tasks(&self) -> Result<()> {
        let client  = self.timer_verticle();

        let storage = self.storage.clone();
        let _ = client.add_timer(
            30_000,
            Some(STORAGE_EXPIRE_INTERVAL),
            AsyncHandler::new(move |_|{
                    let storage = storage.clone();
                    Box::pin(async move {
                        let _ = storage.lock().unwrap().purge();
                })
        }))?;

        //let weak = self.weak.clone();
        let _ = client.add_timer(
            60_000,
            Some(RE_ANNOUNCE_INTERVAL),
            AsyncHandler::new(move |_| {
                //let weak = weak.clone();
                Box::pin(async move {
                   // weak.upgrade().expect("KadNode instance is dropped")
                   //     .persistent_announce().await;
                })
            })
        )?;

        let token_man = self.token_man.clone();
        let _ = client.add_timer(
            TokenManager::TOKEN_TIMEOUT,
            Some(TokenManager::TOKEN_TIMEOUT),
            AsyncHandler::new(move |_| {
                let token_man = token_man.clone();
                Box::pin(async move {
                    token_man.update_token_timestamp();
                })
            })
        )?;
        Ok(())
    }

    pub fn add_listener(&self, listener: Box<dyn ConnectionStatusListener>) {
        self.listeners.lock().unwrap().push(listener);
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_running() {
            return Err(StateError::new(format!("KadNode is already running.")));
        };

        {
            let db_path = self.database_uri.to_str().unwrap();
            let mut locked = self.storage.lock().unwrap();
            locked.open(db_path)?;
            locked.initialize(MAX_VALUE_AGE, MAX_PEER_AGE)?;
        }

        let options = timer_verticle::VerticleOptions{};
        let client = timer_verticle::deploy(options)?;
        *self.timer_verticle.lock().unwrap() = Some(Arc::new(client));

        self.setup_periodic_tasks().await?;

        let listener = Arc::new(DefaultConnectionStatusListener {
            listeners: self.listeners.clone()
        });

        let options = VerticleOptions::default()
            .with_identity(self.identity.identity())
            .with_storage(self.storage.clone())
            .with_tokenman(self.token_man.clone())
            .with_bootstrap(self.cfg.bootstrap_nodes().to_vec())
            .with_datadir(self.data_dir.clone())
            .with_listener(listener);

        let port = self.cfg.port();
        if let Some(host4) = self.cfg.host4() {
            let verticle = dht_verticle::deploy(
                options.clone(),
                Network::IPv4,
                host4.to_string(),
                port
            )?;

            *self.dht4.lock().unwrap() = Some(Arc::new(verticle));
        }
        if let Some(host6) = self.cfg.host6() {
            let verticle = dht_verticle::deploy(
                options.clone(),
                Network::IPv6,
                host6.to_string(),
                port
            )?;

            *self.dht6.lock().unwrap() = Some(Arc::new(verticle));
        }

        *self.running.lock().unwrap() = true;
        info!("Kademlia node started.");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        debug!("Kademlia node is stopping ....");
        if !self.is_running() {
            return Ok(());
        }
        *self.running.lock().unwrap() = false;

        let dht4 = self.dht4.lock().unwrap().take();
        if let Some(dht) = dht4 {
            let mut c = Arc::try_unwrap(dht).ok().unwrap();
            let _ = c.stop().await;
        }

        let dht6 = self.dht6.lock().unwrap().take();
        if let Some(dht) = dht6 {
            let mut c = Arc::try_unwrap(dht).ok().unwrap();
            let _ = c.stop().await;
        }

        let verticle = self.timer_verticle.lock().unwrap().take();
        if let Some(verticle) = verticle {
            let mut vert = Arc::try_unwrap(verticle).ok().unwrap();
            let _ = vert.stop().await;
        }
        self.storage.lock().unwrap().close();

        info!("Kademlia node stopped.");
        logger::teardown();

        Ok(())
    }

    pub fn id(&self) -> &Id {
        self.identity.id()
    }

    pub fn node_info(&self) -> NodeInfo {
        let dht4 = self.dht4.lock().unwrap().clone();
        let dht6 = self.dht6.lock().unwrap().clone();

        let mut ni = None;
        if let Some(dht) = dht6 {
            ni = Some(dht.ni());
        };
        if let Some(dht) = dht4 {
            ni = Some(dht.ni());
        };
        ni.unwrap()
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
        *self.running.lock().unwrap()
    }

    pub async fn bootstrap_one(&self,  node: &NodeInfo) -> Result<()> {
        self.bootstrap(&[node.clone()]).await
    }

    pub async fn bootstrap(&self, nodes: &[NodeInfo]) -> Result<()> {
        self.check_running()?;

        let cb = async move |dht: Option<Arc<VerticleClient>>| {
            let (promise, future) = Promise::<()>::pair();
            let nodes = nodes.to_vec();

            if let Some(dht) = dht {
                dht.bootstrap(nodes, promise).await;
            } else {
                promise.complete(Ok(()));
            }
            future
        };

        let dht4 = self.dht4.lock().unwrap().clone();
        let dht6 = self.dht6.lock().unwrap().clone();

        let result = tokio::join!(
            cb(dht4),
            cb(dht6)
        );
        for item in [result.0, result.1] {
            let _ = item.await?;
        }
        Ok(())
    }

    pub async fn find_node(
        &self,
        target: &Id,
        lookup_option: Option<LookupOption>
    ) -> Result<JointResult<NodeInfo>>
    {
        self.check_running()?;

        let cb = async move |dht: Option<Arc<VerticleClient>>| {
            let (promise, future) = Promise::<Option<NodeInfo>>::pair();
            let target  = target.clone();
            let option  = self.option(lookup_option);

            if let Some(dht) = dht {
                dht.find_node(target, option, promise).await;
            } else {
                promise.complete(Ok(None));
            }
            future
        };

        let dht4 = self.dht4.lock().unwrap().clone();
        let dht6 = self.dht6.lock().unwrap().clone();

        let result = tokio::select!(
            v = cb(dht4), if dht4.is_some() => (Network::IPv4, v),
            v = cb(dht6), if dht6.is_some() => (Network::IPv6, v),
        );

        let mut joint = JointResult::<NodeInfo>::new();
        if let Some(ni) = result.1.await? {
            joint.set_value(result.0, ni);
        }
        Ok(joint)
    }

    pub async fn find_value(
        &self,
        value_id: &Id,
        expected_seq: i32,
        lookup_option: Option<LookupOption>
    ) -> Result<Option<Value>>
    {
        if expected_seq < -1 {
            return Err(ArgumentError::new(format!(
                "Invalid expected sequence number: {expected_seq}, must be larger than or equal to -1")));
        }

        self.check_running()?;

        let target  = value_id.clone();
        let option  = self.option(lookup_option);
        let dht4    = self.dht4.lock().unwrap().clone();
        let dht6    = self.dht6.lock().unwrap().clone();

        let mut ev = EligibleValue::new(target, expected_seq);

        let value = self.storage.lock().unwrap().get_value(&target)?;
        if let Some(v) = value {
            let is_mutable = v.is_mutable();
            ev.update(v, false);

            if !is_mutable {
                return Ok(ev.value());
            }
            if option != LookupOption::Conservative && !ev.is_empty() {
                return Ok(ev.value());
            }
        }

        let cb = async move |dht: Option<Arc<VerticleClient>>| {
            let (promise, future) = Promise::<Option<Value>>::pair();

            if let Some(dht) = dht {
                dht.find_value(
                    target, expected_seq, option, promise).await;
            } else {
                promise.complete(Ok(None));
            }
            future
        };

        let rc = tokio::select!(
            v = cb(dht4), if dht4.is_some() => v,
            v = cb(dht6), if dht6.is_some() => v,
        );

        if let Some(value) = rc.await? {
            ev.update(value, true);
        }

        if !ev.is_empty() && ev.is_latest() {
            let _ = self.storage.lock().unwrap().put_value(
                ev.value().unwrap(), false
            );
        }

        Ok(ev.value())
    }

    pub async fn find_peer(
        &self,
        peer_id: &Id,
        expected_seq: i32,
        expected_count: usize,
        lookup_option: Option<LookupOption>
    ) -> Result<Vec<PeerInfo>>
    {
        if expected_seq < -1 {
            return Err(ArgumentError::new(format!(
                "Invalid expected sequence number: {expected_seq}, must be larger than or equal to -1")));
        }
        if expected_count == 0 {
            return Err(ArgumentError::new(format!(
                "Invalid expected count: {expected_count}, must be larger than 0")));
        }
        self.check_running()?;

        let target  = peer_id.clone();
        let option  = self.option(lookup_option);
        let dht4    = self.dht4.lock().unwrap().clone();
        let dht6    = self.dht6.lock().unwrap().clone();

        let mut ep = EligiblePeers::new(
            target, expected_seq, expected_count);

        let peers = self.storage.lock().unwrap().get_peers_with_expected_seq(
                &target, expected_seq, expected_count as i32)?;

        ep.add(peers, false);
        ep.prune();

        if !ep.is_empty() {
            if option == LookupOption::Local {
                return Ok(ep.peers())
            }
            if option  != LookupOption::Conservative &&
                expected_seq >= 0 && ep.reached_capacity() {
                return Ok(ep.peers())
            }
        }

        let cb = async move |dht: Option<Arc<VerticleClient>>| {
            let (promise, future) = Promise::<Vec<PeerInfo>>::pair();

            if let Some(dht) = dht {
                dht.find_peer(
                    target, expected_seq , expected_count, option, promise).await;
            } else {
                promise.complete(Ok(Vec::new()));
            }
            future
        };

        let rc = tokio::select!(
            v = cb(dht4), if dht4.is_some() => v,
            v = cb(dht6), if dht6.is_some() => v,
        );

        ep.add(rc.await?, true);
        ep.prune();

        if !ep.is_empty() && ep.is_latest() {
            let _ = self.storage.lock().unwrap().put_peers(ep.peers());
        }
        Ok(ep.peers())
    }

    pub async fn store_value(
        &self,
        value: &Value,
        expected_seq: i32,
        persistent: bool
    ) -> Result<()>
    {
        if !value.is_valid() {
            return Err(ArgumentError::new("The value failed validation."));
        }
        if expected_seq < -1 {
            return Err(ArgumentError::new(format!(
                "Invalid expected sequence number: {expected_seq}, must be larger than or equal to -1")));
        }
        self.check_running()?;

        let value_id = value.id();
        let result = self.storage.lock().unwrap().get_value(&value_id)?;
        if let Some(ref existing) = result {
            let _  = check_value_validity(existing, value, expected_seq)?;
        };

        // store the value in local node.
        let _ = self.storage.lock().unwrap().put_value(
            value.clone(), persistent
        )?;

        // store the value to the network.
        let dht4 = self.dht4.lock().unwrap().clone();
        let dht6 = self.dht6.lock().unwrap().clone();

        let cb = async move|dht: Option<Arc<VerticleClient>>| {
            let (promise, future) = Promise::<()>::pair();
            let value   = value.clone();

            if let Some(dht) = dht {
                dht.store_value(value, expected_seq, promise).await;
            } else {
                promise.complete(Ok(()));
            }
            future
        };
        let result = tokio::join!(
            cb(dht4),
            cb(dht6),
        );

        for item in [result.0, result.1] {
            let _ = item.await?;
        }

        let _ = self.storage.lock().unwrap().update_value_announced_time(&value_id);
        Ok(())
    }

    pub async fn announce_peer(
        &self,
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

        let result = self.storage.lock().unwrap().get_peer(
            peer.id(), peer.fingerprint()
        )?;

        // check the peer validity.
        if let Some(ref existing) = result {
            let _ = check_peer_validity(existing, peer, expected_seq)?;
        }

        // store the new peer locally.
        let _ = self.storage.lock().unwrap().put_peer(
            peer.clone(), persistent
        )?;

        // announce the peer to the network.
        let dht4 = self.dht4.lock().unwrap().clone();
        let dht6 = self.dht6.lock().unwrap().clone();

        let cb = async move |dht: Option<Arc<VerticleClient>>| {
            let (promise, future) = Promise::<()>::pair();
            let peer = peer.clone();

            if let Some(dht) = dht {
                dht.announce_peer(peer, expected_seq, promise).await;
            } else {
                promise.complete(Ok(()));
            }
            future
        };

        let result = tokio::join!(
            cb(dht4),
            cb(dht6),
        );
        for item in [result.0, result.1] {
            let _ = item.await?;
        }

        let _ = self.storage.lock().unwrap()
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

fn check_value_validity(old: &Value, new: &Value, expected_seq: i32) -> Result<()> {
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

fn check_peer_validity(old: &PeerInfo, new: &PeerInfo, expected_seq: i32) -> Result<()> {
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

struct DefaultConnectionStatusListener {
    listeners: Arc<Mutex<Vec<Box<dyn ConnectionStatusListener>>>>
}

impl ConnectionStatusListener for DefaultConnectionStatusListener {
    fn status_changed(&self,
        network: Network,
        new_status: ConnectionStatus,
        old_status: ConnectionStatus,
    ) {
        info!("Connection status changed for DHT{{{}}}: {}->{}", network, old_status, new_status);
        let locked = self.listeners.lock().unwrap();
        for l in locked.iter() {
            l.status_changed(network, new_status, old_status);
        }
    }
    fn connecting(&self, network: Network) {
        info!("Connecting to DHT{{{}}}...", network);
        let locked = self.listeners.lock().unwrap();
        for l in locked.iter() {
            l.connecting(network);
        }
    }
    fn connected(&self, network: Network) {
        info!("Connected to DHT{{{}}}.", network);
        let locked = self.listeners.lock().unwrap();
        for l in locked.iter() {
            l.connected(network);
        }
    }
    fn disconnected(&self, network: Network) {
        info!("Disconnected from DHT{{{}}}.", network);
        let locked = self.listeners.lock().unwrap();
        for l in locked.iter() {
            l.disconnected(network);
        }
    }
}

unsafe impl Send for Node {}
unsafe impl Sync for Node {}
