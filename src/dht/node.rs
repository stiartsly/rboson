use std::ptr;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::Read;
use std::net::SocketAddr;
use std::collections::LinkedList;
use std::thread::{self, JoinHandle};
use std::{fs, fs::File, io::Write};
use std::sync::{Arc, Mutex};
use log::{error, info};

use crate::{
    create_dirs,
    Id,
    id::MIN_ID,
    Error,
    error::Result,
    NodeInfo,
    PeerInfo,
    Value,
    Network,
    signature,
    cryptobox,
    JointResult,
    Identity,
    CryptoContext,
    core::logger,
    core::config::Config,
};

use crate::dht::{
    node_status::NodeStatus,
    LookupOption,
    constants,
    node_runner,
    node_runner::NodeRunner,
    crypto_cache::CryptoCache,
    bootstrap_channel::BootstrapChannel,
    future::{
        Cmd,
        Command,
        CmdFuture,
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
    }
};

pub struct Node {
    nodeid: Id,
    port: u16,

    crypto_context: Arc<Mutex<CryptoCache>>,
    bootstr_channel: Arc<Mutex<BootstrapChannel>>,
    command_channel: Arc<Mutex<LinkedList<Command>>>,

    signature_keypair : signature::KeyPair,
    encryption_keypair: cryptobox::KeyPair,

    option: Mutex<LookupOption>,
    status: Mutex<NodeStatus>,
    storage_path: String,

    thread: Mutex<Option<JoinHandle<()>>>,    // working thread.
    quit: Arc<Mutex<bool>>,            // notification handle to quit from working thread.

    addrs: JointResult<SocketAddr>,
}

impl Node {
    pub fn new(cfg: &Box<dyn Config>) -> Result<Self> {
        logger::setup(cfg.log_level(), cfg.log_file().as_deref());

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

        let signature_keypair = get_keypair(&path).map_err(|e| {
            error!("Acquire keypair from {} for DHT node error: {}", path, e);
            e
        })?;

        let encryption_keypair = cryptobox::KeyPair::try_from(&signature_keypair).unwrap();

        let nodeid = {
            let id = Id::from(signature_keypair.public_key());
            let id_path = path.clone() + "id";
            store_nodeid(&id_path, &id).map_err(|e| {
                error!("Persisting node Id data error {}", e);
                return e
            }).ok().unwrap();
            info!("Current DHT node Id: {}", id);
            id
        };

        let port = {
            let port = cfg.port();
            match port > 0 && port < u16::MAX {
                true => port,
                false => constants::DEFAULT_DHT_PORT
            }
        };

        let mut addrs = JointResult::new();
        if let Some(addr4) = cfg.addr4() {
            addrs.set_value(Network::IPv4, addr4.clone());
        }
        if let Some(addr6) = cfg.addr6() {
            addrs.set_value(Network::IPv6, addr6.clone());
        }

        let mut bootstrap_channel = BootstrapChannel::new();
        bootstrap_channel.push_nodes(cfg.bootstrap_nodes().to_vec());

        Ok(Node {
            nodeid,
            port,

            crypto_context: Arc::new(Mutex::new(CryptoCache::new(encryption_keypair.clone()))),
            bootstr_channel: Arc::new(Mutex::new(bootstrap_channel)),
            command_channel: Arc::new(Mutex::new(LinkedList::new())),

            signature_keypair,
            encryption_keypair,

            status: Mutex::new(NodeStatus::Stopped),
            option: Mutex::new(LookupOption::Conservative),
            storage_path: path,

            thread: Mutex::new(None),
            quit: Arc::new(Mutex::new(false)),
            addrs,
        })
    }

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
        let cmds    = self.command_channel.clone();
        let ctx     = self.crypto_context.clone();
        let quit    = self.quit.clone();
        let thread  = thread::spawn(move || {
            let runner = Rc::new(RefCell::new(NodeRunner::new(
                path,
                keypair,
                addrs,
                cmds,
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

    pub fn is_running(&self) -> bool {
        let status_ptr: *const NodeStatus = &(*self.status.lock().unwrap());
        unsafe {
            ptr::read_volatile(status_ptr) == NodeStatus::Running
        }
    }

    pub fn bootstrap(&self, node: &NodeInfo) {
        self.bootstr_channel.lock()
            .expect("Locking failure")
            .push(node.clone());
    }

    pub fn bootstrap_nodes(&self, nodes: &[NodeInfo]) {
        self.bootstr_channel.lock()
            .expect("Locking failure")
            .push_nodes(nodes.to_vec());
    }

    pub const fn id(&self) -> &Id {
        &self.nodeid
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn is_self(&self, id: &Id) -> bool {
        self.id() == id
    }

    pub fn set_lookup_option(&self, option: LookupOption) {
        *self.option.lock().unwrap() = option;
    }

    pub fn lookup_option(&self) -> LookupOption {
        self.option.lock().unwrap().clone()
    }

    pub async fn find_node(&self,
        target: &Id,
        option: Option<&LookupOption>
    ) -> Result<JointResult<NodeInfo>> {
        if target == &MIN_ID {
            return Err(Error::Argument(format!("Invalid target node id {}", target)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let default_opt = self.option.lock().unwrap();
        let opt = option.unwrap_or(&default_opt);
        let arc = Arc::new(Mutex::new(FindNodeCmd::new(target, opt)));
        let cmd = Command::FindNode(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn find_value(&self,
        value_id: &Id,
        option: Option<&LookupOption>
    ) -> Result<Option<Value>> {
        if value_id == &MIN_ID {
            return Err(Error::Argument(format!("Invalid value id {}", value_id)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let default_opt = self.option.lock().unwrap();
        let opt = option.unwrap_or(&default_opt);
        let arc = Arc::new(Mutex::new(FindValueCmd::new(value_id, opt)));
        let cmd = Command::FindValue(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn find_peer(&self,
        peer_id: &Id,
        expected_seq: Option<usize>,
        option: Option<&LookupOption>
    ) -> Result<Vec<PeerInfo>> {
        if peer_id == &MIN_ID {
            return Err(Error::Argument(format!("Invalid peer id {}", peer_id)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let default_opt = *self.option.lock().unwrap();
        let opt = option.unwrap_or(&default_opt);
        let seq = expected_seq.unwrap_or(0);
        let arc = Arc::new(Mutex::new(FindPeerCmd::new(
            peer_id,
            seq,
            opt
        )));
        let cmd = Command::FindPeer(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn store_value(&self,
        value: &Value,
        persistent: Option<bool>
    ) -> Result<()> {
        if !value.is_valid() {
            return Err(Error::Argument(format!("Invalid value")));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let persistent = persistent.unwrap_or(false);
        let arc = Arc::new(Mutex::new(StoreValueCmd::new(value, persistent)));
        let cmd = Command::StoreValue(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn announce_peer(&self,
        peer: &PeerInfo,
        persistent: Option<bool>
    ) -> Result<()> {
        if !peer.is_valid() {
            return Err(Error::Argument(format!("Invalid peer")));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let persistent = persistent.unwrap_or(false);
        let arc = Arc::new(Mutex::new(AnnouncePeerCmd::new(peer, persistent)));
        let cmd = Command::AnnouncePeer(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn value(&self, value_id: &Id) -> Result<Option<Value>> {
        if value_id == &MIN_ID {
            return Err(Error::Argument(format!("Invalid value id {}", value_id)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let arc = Arc::new(Mutex::new(GetValueCmd::new(value_id)));
        let cmd = Command::GetValue(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn remove_value(&self, value_id: &Id) -> Result<()> {
        if value_id == &MIN_ID {
            return Err(Error::Argument(format!("Invalid value id {}", value_id)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let arc = Arc::new(Mutex::new(RemoveValueCmd::new(value_id)));
        let cmd = Command::RemoveValue(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn value_ids(&self) -> Result<Vec<Id>> {
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let arc = Arc::new(Mutex::new(GetValueIdsCmd::new()));
        let cmd = Command::GetValueIds(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn peer(&self, peer_id: &Id) -> Result<Option<PeerInfo>> {
        if peer_id == &MIN_ID {
            return Err(Error::Argument(format!("Invalid peer id {}", peer_id)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let arc = Arc::new(Mutex::new(GetPeerCmd::new(peer_id)));
        let cmd = Command::GetPeer(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn remove_peer(&self, peer_id: &Id) -> Result<()> {
        if peer_id == &MIN_ID {
            return Err(Error::Argument(format!("Invalid peer id {}", peer_id)));
        }
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let arc = Arc::new(Mutex::new(RemovePeerCmd::new(peer_id)));
        let cmd = Command::RemovePeer(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    pub async fn peer_ids(&self) -> Result<Vec<Id>> {
        if !self.is_running() {
            return Err(Error::State(format!("DHT node {} is not running", self.nodeid)))
        }

        let arc = Arc::new(Mutex::new(GetPeerIdsCmd::new()));
        let cmd = Command::GetPeerIds(arc.clone());

        self.command_channel.lock().unwrap().push_back(cmd.clone());
        match CmdFuture::new(cmd.clone()).await {
            Ok(_) => arc.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }
}

fn get_keypair(path: &str) -> Result<signature::KeyPair> {
    create_dirs(path).map_err(|e| {
        return Error::State(format!("Checking persistence error: {}", e));
    }).ok().unwrap();

    let keypath = path.to_string() + "key";
    let keypair;

    match fs::metadata(&keypath) {
        Ok(metadata) => {
            // Loading key from persistence.
            if metadata.is_dir() {
                return Err(Error::State(format!("Bad file path {} for key storage.", keypath)));
            };
            keypair = load_key(&keypath)
                .map_err(|e| Error::from(e))?
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
    type IdentityObject = Node;

    fn id(&self) -> &Id {
        &self.nodeid
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        signature::sign(data, signature, self.signature_keypair.private_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        signature::sign_into(data, self.signature_keypair.private_key())
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        signature::verify(data, signature, self.signature_keypair.public_key())
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        let mut cache = self.crypto_context.lock().unwrap();
        cache.get(recipient).lock().unwrap().ctx_mut().encrypt(plain, cipher)
    }

    fn encrypt_into(&self, recipient: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut cache = self.crypto_context.lock().unwrap();
        cache.get(recipient).lock().unwrap().ctx_mut().encrypt_into(plain)
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        let mut cache = self.crypto_context.lock().unwrap();
        cache.get(sender).lock().unwrap().ctx_mut().decrypt(cipher, plain)
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        let mut cache = self.crypto_context.lock().unwrap();
        cache.get(sender).lock().unwrap().ctx_mut().decrypt_into(cipher)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        Ok(CryptoContext::from_private_key(
            id.clone(),
            self.encryption_keypair.private_key()
        ))
    }
}

use std::str;
fn load_key(path: &str) -> Result<signature::KeyPair> {
    let mut fp = match File::open(path) {
        Ok(v) => v,
        Err(e) => return Err(Error::Io(
            format!("Openning key file error: {}", e))),
    };

    let mut buf = Vec::new();
    if let Err(e) = fp.read_to_end(&mut buf) {
        return Err(Error::Io(format!("Reading key error: {}", e)));
    };

    let sk: signature::PrivateKey = str::from_utf8(&buf).map_err(|e| {
        return Error::State(format!("Key file is not UTF-8: {}", e));
    })?.try_into().map_err(|e| {
        return Error::State(format!("Key file is not a valid key: {}", e));
    })?;

    Ok(signature::KeyPair::from(&sk))
}

fn store_key(path: &str, keypair: &signature::KeyPair) -> Result<()> {
    let mut fp = match File::create(path) {
        Ok(v) => v,
        Err(e) => return Err(Error::Io(
            format!("Creating key file error: {}", e))),
    };

    let result = fp.write_all(keypair.private_key().to_string().as_bytes());
    if let Err(e) = result {
        return Err(Error::Io(format!("Writing key error: {}", e)));
    }
    Ok(())
}

fn store_nodeid(path: &str, id: &Id) -> Result<()> {
    let mut fp = match File::create(path) {
        Ok(v) => v,
        Err(e) => return Err(Error::Io(
            format!("Creating Id file error: {}", e))),
    };

    let result = fp.write_all(id.to_base58().as_bytes());
    if let Err(e) = result {
        return Err(Error::Io(format!("Writing ID error: {}", e)));
    };
    Ok(())
}
