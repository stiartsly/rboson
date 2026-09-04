use std::{
    cell::Cell,
    sync::{Arc, Mutex},
    net::{SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    io::Read,
    fs::File,
};
use log::{error, warn, debug, info};

use crate::{
    Id,
    PeerInfo,
    Result,
    errors::{NetworkError, ArgumentError},
    dht::Node
};

use super::{
    options::Options,
    verticle::{self, VerticleClient}
};

pub struct ActiveProxyClient {
    options             : Options,
    node                : Option<Arc<Node>>,
    cache_file          : PathBuf,

    service_peerid      : Id,
    service_peer        : Arc<Mutex<Option<PeerInfo>>>,
    service_endpoint    : Option<String>,

    upstream_endpoint   : String,
    upstream_addr       : SocketAddr,

    verticle            : Cell<Option<VerticleClient>>,
    running             : Cell<bool>,
}

impl ActiveProxyClient {
    pub fn new(node: Option<Arc<Node>>, options: Options) -> Result<Arc<Self>> {
        let path = PathBuf::from(options.data_dir()).join("activeproxy");
        if let Err(e) = std::fs::create_dir_all(&path) {
            return Err(ArgumentError::new(format!(
                "Failed to create ActiveProxy cache dir {}: {e}",
                path.display()
            )));
        }

        if node.is_none() &&
            options.service_peer().is_none() &&
            options.service_host().is_none() {
            return Err(ArgumentError::new(
                "ActiveProxy requires either a DHT node or a specified service peer or service host",
            ));
        }

        let upstream_endpoint = format!(
            "{}{}:{}", options.upstream_scheme(), options.upstream_host(), options.upstream_port()
        );
        let rest = upstream_endpoint.strip_prefix("tcp://").unwrap_or(&upstream_endpoint);
        let upstream_sockaddr = rest.to_socket_addrs()
            .map_err(|e| {
                error!("Failed to resolve address '{rest}', network error: {e}");
                ArgumentError::new(format!("Bad upstream address: {rest}"))
            })?
            .next()
            .ok_or_else(|| {
                error!("No valid address found for '{rest}', network error!!!");
                NetworkError::new(format!("No valid address found for '{rest}'!"))
            })?;

        let mut endpoint = None;
        if let Some(host) = options.service_host() {
            endpoint = Some(format!("{}:{}", host, options.service_port()));
        } else if let Some(peer) = options.service_peer() {
            endpoint = Some(peer.endpoint().to_string());
        } else if let Some(peer) = load_peer(&path, options.service_peerid()) {
            debug!("ActiveProxy loaded peer {} from cached file.", peer.id());
            endpoint = Some(peer.endpoint().to_string());
        } else if node.is_none() {
            return Err(ArgumentError::new(
                "ActiveProxy requires a DHT node to lookup a service peer information.",
            ));
        }

        Ok(Arc::new(Self {
            node,
            cache_file          : path.to_path_buf(),
            service_peerid      : options.service_peerid().clone(),
            service_peer        : Arc::new(Mutex::new(options.service_peer().cloned())),
            service_endpoint    : endpoint,
            upstream_endpoint,
            upstream_addr       : upstream_sockaddr,
            verticle            : Cell::new(None),
            running             : Cell::new(false),
            options
        }))
    }

    pub fn node(&self) -> Option<Arc<Node>> {
        self.node.clone()
    }

    pub fn node_id(&self) -> Option<Id> {
        self.node.as_ref().map(|n| n.id().clone())
    }

    pub fn upstream_host(&self) -> &str {
        &self.options.upstream_host()
    }

    pub fn upstream_port(&self) -> u16 {
        self.options.upstream_port()
    }

    pub fn upstream_endpoint(&self) -> &str {
        &self.upstream_endpoint
    }

    pub fn upstream_socketaddr(&self) -> &SocketAddr {
        &self.upstream_addr
    }

    pub fn service_peerid(&self) -> &Id {
        &self.service_peerid
    }

    pub fn service_peer(&self) -> Option<PeerInfo> {
        self.service_peer.lock().unwrap().clone()
    }

    pub fn service_endpoint(&self) -> Option<String> {
        if let Some(endpoint) = self.service_endpoint.clone() {
            Some(endpoint)
        } else if let Some(peer) = self.service_peer() {
            Some(peer.endpoint().to_string())
        } else {
            None
        }
    }

    pub async fn start(&self) -> Result<()> {
        let _ = match self.service_peer() {
            Some(_) => {},
            _ => {
                let node = self.node().unwrap();
                let peer = super::utils::lookup_peer(node, self.service_peerid()).await?;
                if let Some(peer) = peer {
                    super::utils::save_peer(self.cache_file.as_path(), &peer);
                    *self.service_peer.lock().unwrap() = Some(peer);
                }
            }
        };

        let options = verticle::VerticleOptions::new(
            self.node(),
            self.service_peerid().clone(),
            //self.service_peer(),
            self.service_endpoint().unwrap(),
            self.upstream_socketaddr().clone(),
            self.options.user_id().clone(),
            self.options.device_key().private_key().clone(),
        );

        let client = verticle::deploy(options).map_err(|e| {
            ArgumentError::new(format!("Failed to deploy ActiveProxy verticle: {e}"))
        })?;

        client.start().await?;

        self.running.set(true);
        self.verticle.set(Some(client));
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.get()
    }

    pub async fn stop(&self) {
        debug!("ActiveProxy instance is stopping ....");
        if !self.is_running() {
            return;
        }

        if let Some(mut v) = self.verticle.replace(None) {
            let _ = v.stop().await;
        }

        info!("ActiveProxy instance stopped.");
    }
}

fn load_peer(path: &Path, peerid: &Id) -> Option<PeerInfo> {
    let mut buf = vec![];
    let _ = File::open(path).map(|mut fp| {
        _ = fp.read_to_end(&mut buf);
    }).map_err(|e| {
        warn!("Failed to open cached file {} with error: {e}.",
            path.display());
        None::<File>
    }).ok()?;

    let peer: PeerInfo = serde_json::from_reader(buf.as_slice()).map_err(|e| {
        warn!("Failed to parse data from cached file {} with error: {e} - \
            cached file might be broken", path.display());
        None::<PeerInfo>
    }).ok()?;

    if !peer.is_valid() || peer.id() != peerid {
        warn!("The cached peer {} is invalid or outdated since it does not match the expected {}", peer.id(), peerid);
        return None;
    }
    Some(peer)
}
