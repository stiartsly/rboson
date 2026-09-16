use log::{debug, error, info};
use std::{
    cell::Cell,
    net::{SocketAddr, ToSocketAddrs},
    sync::{Arc, Mutex},
};

use crate::{
    dht::Node,
    errors::{ArgumentError, NetworkError},
    Id, PeerInfo, Result,
};

use super::{
    options::Options,
    verticle::{self, VerticleClient},
};

pub struct ActiveProxyClient {
    options: Options,
    node: Option<Arc<Node>>,

    service_peerid: Id,
    service_peer: Arc<Mutex<Option<PeerInfo>>>,
    service_endpoint: Option<String>,

    upstream_endpoint: String,
    upstream_addr: SocketAddr,

    verticle: Cell<Option<VerticleClient>>,
    running: Cell<bool>,
}

impl ActiveProxyClient {
    pub fn new(node: Option<Arc<Node>>, options: Options) -> Result<Arc<Self>> {
        options.check_valid()?;

        if node.is_none() && options.service_peer().is_none() && options.service_host().is_none() {
            return Err(ArgumentError::new(
                "ActiveProxy requires either a DHT node or a specified service peer or service host",
            ));
        }

        let upstream_endpoint = format!(
            "{}{}:{}",
            options.upstream_scheme(),
            options.upstream_host(),
            options.upstream_port()
        );
        let rest = upstream_endpoint
            .strip_prefix("http://")
            .unwrap_or(&upstream_endpoint);
        let upstream_sockaddr = rest
            .to_socket_addrs()
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
        if let Some(peer) = options.service_peer() {
            if !peer.is_valid() {
                return Err(ArgumentError::new(format!(
                    "Invalid ActiveProxy service peer {}",
                    peer.id()
                )));
            }
            endpoint = Some(peer.endpoint().to_string());
        } else if let Some(host) = options.service_host() {
            endpoint = Some(format!("{}:{}", host, options.service_port()));
        } else if node.is_none() {
            return Err(ArgumentError::new(
                "ActiveProxy requires a DHT node to lookup a service peer information.",
            ));
        }

        Ok(Arc::new(Self {
            node,
            service_peerid: options.service_peerid().clone(),
            service_peer: Arc::new(Mutex::new(options.service_peer().cloned())),
            service_endpoint: endpoint,
            upstream_endpoint,
            upstream_addr: upstream_sockaddr,
            verticle: Cell::new(None),
            running: Cell::new(false),
            options,
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
        if self.service_endpoint().is_none() {
            let node = self.node().ok_or_else(|| {
                ArgumentError::new(
                    "ActiveProxy requires a DHT node to lookup service peer information"
                )
            })?;
            let peer = node
                .find_peer(self.service_peerid(), -1, 4, None)
                .await?
                .into_iter()
                .find(|peer| peer.id() == self.service_peerid() && peer.is_valid())
                .ok_or_else(|| {
                    NetworkError::new(format!(
                        "No valid peer found for ActiveProxy service {}",
                        self.service_peerid()
                    ))
                })?;
            *self.service_peer.lock().unwrap() = Some(peer);
        }

        let service_endpoint = self.service_endpoint().ok_or_else(|| {
            NetworkError::new(format!(
                "No endpoint available for ActiveProxy service peer {}",
                self.service_peerid()
            ))
        })?;

        let options = verticle::VerticleOptions::new(
            self.node(),
            self.service_peerid().clone(),
            service_endpoint,
            self.upstream_socketaddr().clone(),
            self.upstream_endpoint().to_string(),
            self.options.user_id().clone(),
            self.options.device_key().private_key().clone(),
            self.options.is_name_access_enabled(),
            self.node.is_some() && self.options.is_announce_peer_enabled(),
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
        self.running.set(false);

        info!("ActiveProxy instance stopped.");
    }
}
