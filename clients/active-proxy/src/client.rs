use log::{debug, error, info};
use std::{
    cell::Cell,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use crate::{
    core::logger,
    errors::{ArgumentError, NetworkError},
    Id, Result,
};

use super::{
    options::Options,
    verticle::{self, VerticleClient},
};

pub struct ActiveProxyClient {
    options: Options,
    service_peerid: Id,
    service_endpoint: String,

    upstream_endpoint: String,
    upstream_addr: SocketAddr,

    verticle: Cell<Option<VerticleClient>>,
    running: Cell<bool>,
}

impl ActiveProxyClient {
    pub fn new(options: Options) -> Result<Arc<Self>> {
        if log::max_level() == log::LevelFilter::Off {
            logger::setup(options.log_level(), options.log_file());
            if options.log_console() {
                logger::enable_console_output();
            } else {
                logger::disable_console_output();
            }
        }

        if options.service_host().trim().is_empty() {
            return Err(ArgumentError::new("ActiveProxy service host is empty"));
        }
        if options.service_port() == 0 {
            return Err(ArgumentError::new("ActiveProxy service port is not set"));
        }
        if options.upstream_host().trim().is_empty() {
            return Err(ArgumentError::new("ActiveProxy upstream host is empty"));
        }
        if options.upstream_port() == 0 {
            return Err(ArgumentError::new("ActiveProxy upstream port is not set"));
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

        let endpoint = format!("{}:{}", options.service_host(), options.service_port());

        Ok(Arc::new(Self {
            service_peerid: options.service_peerid().clone(),
            service_endpoint: endpoint,
            upstream_endpoint,
            upstream_addr: upstream_sockaddr,
            verticle: Cell::new(None),
            running: Cell::new(false),
            options,
        }))
    }

    pub fn options(&self) -> &Options {
        &self.options
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

    pub fn service_endpoint(&self) -> Option<String> {
        Some(self.service_endpoint.clone())
    }

    pub async fn start(&self) -> Result<()> {
        if self.running.get() {
            return Err(ArgumentError::new(
                "ActiveProxy instance is already running",
            ));
        }

        let options = verticle::VerticleOptions::new(
            self.service_peerid().clone(),
            self.service_endpoint.clone(),
            self.upstream_socketaddr().clone(),
            self.upstream_endpoint().to_string(),
            self.options.user_id().clone(),
            self.options.device_private_key().clone(),
            self.options.is_name_access_enabled(),
            self.options.is_announce_peer_enabled(),
            self.options.announce_peer_handler(),
        );

        let mut client = verticle::deploy(options).map_err(|e| {
            ArgumentError::new(format!("Failed to deploy ActiveProxy verticle: {e}"))
        })?;

        if let Err(e) = client.start().await {
            if let Err(stop_error) = client.stop().await {
                error!("Failed to clean up ActiveProxy verticle after startup error: {stop_error}");
            }
            return Err(e);
        }

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
            if let Err(e) = v.stop().await {
                error!("Failed to stop ActiveProxy verticle cleanly: {e}");
            }
        }
        self.running.set(false);

        info!("ActiveProxy instance stopped.");
    }
}
