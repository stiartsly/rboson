use std::sync::{Arc, Mutex};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use std::fs::File;

use tokio::runtime::Runtime;
use rand::seq::SliceRandom;
use log::{error, warn, info, debug};

use crate::{
    Id,
    PeerInfo,
    NodeInfo,
    signature,
    Result,
    core::errors::{ArgumentError, StateError},
    dht::Node,
};

use super::{
    managed::ManagedFields,
    worker::{self, ManagedWorker},
};

pub struct ActiveProxyOptions {
    pub cached_dir: PathBuf,
    pub server_peerid: Id,
    pub user_keypair: signature::KeyPair,
    pub peer_keypair: Option<signature::KeyPair>,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub upstream_domain: Option<String>,
}

pub struct ProxyClient {
    node:               Arc<Node>,
    cached_dir:         PathBuf,

    remote_peerid:      Id,
    remote_peer:        Option<Arc<Mutex<PeerInfo>>>,
    remote_node:        Option<Arc<Mutex<NodeInfo>>>,

    upstream_host:      String,
    upstream_port:      u16,
    upstream_endpoint:  String,
    upstream_addr:      SocketAddr,
    upstream_domain:    Option<String>,

    managed:            Arc<Mutex<ManagedFields>>,
    worker:             Arc<Mutex<ManagedWorker>>,
    quit:               Arc<Mutex<bool>>,
}

impl ProxyClient {
    pub fn new(node: Arc<Node>, options: ActiveProxyOptions) -> Result<Self> {
        let upstream_name = format!("{}:{}", options.upstream_host, options.upstream_port);
        let upstream_addr = upstream_name.to_socket_addrs()
            .map_err(|e| {
                error!("Failed to resolve address '{upstream_name}', network error: {e}");
                ArgumentError::new(format!("Bad upstream {upstream_name}"))
            })?
            .next()
            .ok_or_else(|| {
                error!("No valid address found for '{upstream_name}', network error!!!");
                ArgumentError::new("Network error!")
            })?;

        let managed = {
            let mut fields = ManagedFields::new(&options.user_keypair);
            fields.peer_keypair  = options.peer_keypair;
            fields.upstream_addr = Some(upstream_addr.clone());
            fields.upstream_name = Some(upstream_name.clone());
            fields.peer_domain   = options.upstream_domain.clone();

            Arc::new(Mutex::new(fields))
        };

        let peerid = options.server_peerid.clone();
        let worker = Arc::new(Mutex::new(ManagedWorker::new(
            options.cached_dir.clone(),
            node.clone(),
            managed.clone(),
            peerid.clone(),
        )));

        Ok(Self {
            node,
            cached_dir: options.cached_dir,

            remote_peerid:  peerid,
            remote_peer:    None,
            remote_node:    None,

            upstream_host:  options.upstream_host,
            upstream_port:  options.upstream_port,
            upstream_endpoint:  upstream_name,
            upstream_addr:  upstream_addr,
            upstream_domain:    options.upstream_domain,

            managed,
            worker,
            quit:           Arc::new(Mutex::new(false)),
        })
    }

    pub fn nodeid(&self) -> Id {
        self.node.id().clone()
    }

    pub fn node(&self) -> Arc<Node> {
        self.node.clone()
    }

    pub fn cached_path(&self) -> &Path {
        self.cached_dir.as_path()
    }

    pub fn upstream_host(&self) -> &str {
        &self.upstream_host
    }

    pub fn upstream_port(&self) -> u16 {
        self.upstream_port
    }

    pub fn upstream_endpoint(&self) -> &str {
        &self.upstream_endpoint
    }

    pub fn upstream_addr(&self) -> &SocketAddr {
        &self.upstream_addr
    }

    pub fn domain_name(&self) -> Option<&str> {
        self.upstream_domain.as_deref()
    }

    pub fn remote_peerid(&self) -> &Id {
        &self.remote_peerid
    }

    pub fn remote_peer(&self) -> Option<PeerInfo> {
        self.remote_peer.as_ref().map(|v|v.lock().unwrap().clone())
    }

    pub fn remote_node(&self) -> Option<NodeInfo> {
        self.remote_node.as_ref().map(|v|v.lock().unwrap().clone())
    }

    pub fn start(&self) -> Result<()> {
        let result = load_peer(self.cached_path(), self.remote_peerid()).or_else(||{
            if self.cached_path().exists() {
                _ = std::fs::remove_file(self.cached_path());
            }

            Runtime::new().unwrap().block_on(async {
                lookup_peer(self.node(), self.remote_peerid()).await
            }).map(|v| {
                _ = save_peer(self.cached_path(), v.clone());
                v
            })
        }).unzip();

        let Some(peer) = result.0 else {
            error!("No available peers with peer ID {} were found.", self.remote_peerid);
            return Err(StateError::new(format!("No available peers with peerid {} found", self.remote_peerid)));
        };

        let Some(node) = result.1 else {
            error!("No available nodes hosting peer ID {} were found.", self.remote_peerid);
            return Err(StateError::new(format!("No available nodes hosting peerid {} found", self.remote_peerid)));
        };

        let remote_addr = peer.endpoint().to_socket_addrs().ok().and_then(|mut addrs| addrs.next())
            .unwrap_or_else(|| SocketAddr::new(node.ip(), 0));
        info!("ActiveProxy found the peer serivce {} on server {}.", peer.id(), remote_addr);

        if let Ok(mut managed) = self.managed.lock() {
            managed.remote_peer = Some(Arc::new(Mutex::new(peer)));
            managed.remote_node = Some(Arc::new(Mutex::new(node)));
            managed.remote_addr = Some(remote_addr);
            managed.remote_name = Some(remote_addr.to_string());
        }

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let worker = self.worker.clone();
        let quit = self.quit.clone();
        rt.block_on(async {
            _ = worker::run_loop(worker, quit).await
        });

        Ok(())
    }

    pub fn stop(&self) {
        // TODO:
    }
}

fn load_peer(path: &Path, peerid: &Id) -> Option<(PeerInfo, NodeInfo)> {
    info!("ActiveProxy is trying to load peer {peerid} and its host node from cached file...");

    let mut buf = vec![];
    let _ = File::open(path).map(|mut fp| {
        _ = fp.read_to_end(&mut buf);
    }).map_err(|e| {
        warn!("Failed to open cached file {} with error: {e}.", path.display());
        None::<File>
    }).ok()?;

    let cached: (PeerInfo, NodeInfo) = ciborium::de::from_reader(buf.as_slice()).map_err(|e| {
        warn!("Failed to parse data from cached file {} with error: {e} - \
               cached file might be broken", path.display());
        None::<(PeerInfo, NodeInfo)>
    }).ok()?;
    let (peer, node) = cached;

    if !peer.is_valid() || peer.id() != peerid {
        warn!("The cached peer {} is invalid or outdated since it does not match the expected {peerid}", peer.id());
        return None;
    }

    info!("ActiveProxy loaded peer {} and its host node {} from cached file.",
        peer.id(),
        node.id()
    );

    Some((peer, node))
}

pub(crate) fn save_peer(path: &Path, input: (PeerInfo, NodeInfo)) {
    debug!("ActiveProxy is trying to persist peer {} and its host node into cached file...",
        input.0.id());

    let mut buf = vec![];
    if let Err(e) = ciborium::ser::into_writer(&input, &mut buf) {
        warn!("Failed to persist peer {} and its host node error {e}", input.0.id());
        return;
    }

    _ = File::create(path).map(|mut fp| {
        _ = fp.write_all(&buf);
        debug!("ActiveProxy persisted peer {} and its host node to cached file.",
            input.0.id());
    });
}

pub(crate) async fn lookup_peer(node: Arc<Node>, peerid: &Id) -> Option<(PeerInfo, NodeInfo)> {
    info!("ActiveProxy is trying to find peer {} and its host node via DHT network...", peerid);

    let result = node.find_peer(peerid, -1, 4, None).await;
    if let Err(e) = result {
        warn!("Trying to find peer but error: {}, please try it later!!!", e);
        return None;
    }

    let mut peers = result.unwrap();
    if peers.is_empty() {
        warn!("No peers with peerid {} is found at this moment, please try it later!!!", peerid);
        return None;
    }

    debug!("Discovered {} satisfied peers, extracting each node's infomation...", peers.len());

    let mut rng = rand::rng();
    peers.shuffle(&mut rng);
    while let Some(peer) = peers.pop() {
        let Some(nodeid) = peer.nodeid() else {
            continue;
        };
        debug!("Trying to lookup node {} hosting the peer {} ...", nodeid, peerid);

        let result = node.find_node(nodeid, None).await;
        if let Err(e) = result {
            warn!("AcriveProxy failed to find node {}, error: {}", nodeid, e);
            return None;
        }

        let join_result = result.unwrap();
        if join_result.is_empty() {
            warn!("AcriveProxy can't locate node: {}! Go on next ...", nodeid);
            continue;
        }

        let mut node = None;
        if let Some(v6) = join_result.v6() {
            node = Some(v6.clone());
        }
        if let Some(v4) = join_result.v4() {
            node = Some(v4.clone());
        }
        let Some(node) = node else {
            continue;
        };

        info!("ActiveProxy found peer {} and its host node {}.", peer.id(), node.id());
        return Some((peer, node))
    }
    None
}
