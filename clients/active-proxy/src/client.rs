// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use log::{debug, error, info};
use std::{
    cell::Cell,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use boson_api::{
    logger,
    errors::{ArgumentError, NetworkError},
    Id, Result,
};

use super::{
    options::Options,
    verticle::{self, VerticleClient},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Stopped,
    Starting,
    Running,
}

pub struct ActiveProxyClient {
    options: Options,
    service_peerid: Id,
    service_endpoint: String,

    upstream_endpoint: String,
    upstream_addr: SocketAddr,

    verticle: Cell<Option<VerticleClient>>,
    state: Cell<State>,
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

        Self::validate(&options)?;

        let upstream_endpoint = format!(
            "{}{}:{}",
            options.upstream_scheme(),
            options.upstream_host(),
            options.upstream_port()
        );
        let upstream_sockaddr = resolve_upstream(&upstream_endpoint)?;

        let endpoint = format!("{}:{}", options.service_host(), options.service_port());

        Ok(Arc::new(Self {
            service_peerid: options.service_peerid().clone(),
            service_endpoint: endpoint,
            upstream_endpoint,
            upstream_addr: upstream_sockaddr,
            verticle: Cell::new(None),
            state: Cell::new(State::Stopped),
            options,
        }))
    }

    fn validate(options: &Options) -> Result<()> {
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
        Ok(())
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
        if self.state.get() != State::Stopped {
            return Err(ArgumentError::new(
                "ActiveProxy instance is already running",
            ));
        }
        // Claim the slot before the first await so concurrent callers cannot double-start.
        self.state.set(State::Starting);

        match self.deploy_verticle().await {
            Ok(client) => {
                self.verticle.set(Some(client));
                self.state.set(State::Running);
                Ok(())
            }
            Err(e) => {
                self.state.set(State::Stopped);
                Err(e)
            }
        }
    }

    async fn deploy_verticle(&self) -> Result<VerticleClient> {
        let options = verticle::VerticleOptions::new(
            self.service_peerid().clone(),
            self.service_endpoint.clone(),
            self.upstream_addr,
            self.upstream_endpoint.clone(),
            self.options.user_id().clone(),
            self.options.device_private_key().clone(),
            self.options.is_name_access_enabled(),
            self.options.is_announce_peer_enabled(),
            self.options.announce_peer_handler(),
        );

        let mut client = verticle::deploy(options).await.map_err(|e| {
            ArgumentError::new(format!("Failed to deploy ActiveProxy verticle: {e}"))
        })?;

        if let Err(e) = client.start().await {
            if let Err(stop_error) = client.stop().await {
                error!("Failed to clean up ActiveProxy verticle after startup error: {stop_error}");
            }
            return Err(e);
        }
        Ok(client)
    }

    pub async fn stop(&self) {
        debug!("ActiveProxy instance is stopping ....");
        if self.state.get() != State::Running {
            return;
        }

        self.state.set(State::Stopped);
        if let Some(mut v) = self.verticle.take() {
            if let Err(e) = v.stop().await {
                error!("Failed to stop ActiveProxy verticle cleanly: {e}");
            }
        }

        info!("ActiveProxy instance stopped.");
    }
}

fn resolve_upstream(endpoint: &str) -> Result<SocketAddr> {
    let rest = endpoint.strip_prefix("http://").unwrap_or(endpoint);
    Ok(rest
        .to_socket_addrs()
        .map_err(|e| {
            error!("Failed to resolve address '{rest}', network error: {e}");
            ArgumentError::new(format!("Bad upstream address: {rest}"))
        })?
        .next()
        .ok_or_else(|| {
            error!("No valid address found for '{rest}', network error!!!");
            NetworkError::new(format!("No valid address found for '{rest}'!"))
        })?)
}
