use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use serde::Deserialize;
use tokio::sync::Notify;

use boson::{
    Id,
    Network,
    signature,
    dht::{Node, NodeConfig, NodeConfiguration, ConnectionStatus, ConnectionStatusListener},
    activeproxy::{ActiveProxyClient as ActiveProxy, client::ActiveProxyOptions},
};

#[derive(Parser, Debug)]
#[command(name = "launcher")]
#[command(version = "1.0")]
#[command(about = "Boson launcher service", long_about = None)]
struct Options {
    /// The configuration file (YAML)
    #[arg(short, long, value_name = "FILE", default_value = "default.yaml")]
    config: String,
}

/// The `activeproxy:` section of the launcher's YAML config file.
#[derive(Debug, Deserialize)]
struct ActiveProxySection {
    #[serde(rename = "serverPeerId")]
    server_peerid: String,
    #[serde(rename = "peerPrivateKey")]
    peer_private_key: Option<String>,
    #[serde(rename = "upstreamHost")]
    upstream_host: String,
    #[serde(rename = "upstreamPort")]
    upstream_port: u16,
    #[serde(rename = "upstreamDomain")]
    upstream_domain: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LauncherConfig {
    activeproxy: Option<ActiveProxySection>,
}

/// Reads the launcher's own `activeproxy:` section, which sits alongside but
/// outside of `NodeConfiguration`'s schema.
fn load_activeproxy_section(path: &str) -> Option<ActiveProxySection> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str::<LauncherConfig>(&raw).ok()?.activeproxy
}

fn build_activeproxy_options(
    section: ActiveProxySection,
    data_dir: &str,
    user_keypair: signature::KeyPair,
) -> Result<ActiveProxyOptions, String> {
    let server_peerid = Id::try_from(section.server_peerid.as_str())
        .map_err(|e| format!("Invalid activeproxy.serverPeerId: {e}"))?;

    let peer_keypair = section.peer_private_key.as_deref()
        .map(|key| signature::PrivateKey::try_from(key)
            .map(|sk| signature::KeyPair::from(&sk))
            .map_err(|e| format!("Invalid activeproxy.peerPrivateKey: {e}"))
        )
        .transpose()?;

    Ok(ActiveProxyOptions {
        cached_dir: PathBuf::from(data_dir).join("activeproxy.cache"),
        server_peerid,
        user_keypair,
        peer_keypair,
        upstream_host: section.upstream_host,
        upstream_port: section.upstream_port,
        upstream_domain: section.upstream_domain,
    })
}

/// Notifies once the node has connected to the Boson network.
struct ReadyListener(Arc<Notify>);
impl ConnectionStatusListener for ReadyListener {
    fn status_changed(&self, network: Network, new_status: ConnectionStatus, old_status: ConnectionStatus) {
        println!("Connection status changed for network {network}: {old_status}->{new_status}");
    }
    fn connecting(&self, network: Network) {
        println!("Connecting to network {network}...");
    }
    fn connected(&self, network: Network) {
        println!("Connected to network {network}.");
        self.0.notify_one();
    }
    fn disconnected(&self, network: Network) {
        println!("Disconnected from network {network}.");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opts = Options::parse();

    let node_cfg = match NodeConfiguration::load(&opts.config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            exit(1);
        }
    };

    #[cfg(feature = "inspect")]
    node_cfg.dump();

    let data_dir = node_cfg.data_dir().to_string();
    let user_keypair = signature::KeyPair::from(node_cfg.private_key());

    let node = match Node::new(Box::new(node_cfg)) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Creating Node instance error: {e}");
            exit(1);
        }
    };

    let ready = Arc::new(Notify::new());
    node.add_listener(ReadyListener(ready.clone()));

    if let Err(e) = node.start().await {
        eprintln!("Starting node failed: {e}");
        exit(1);
    }
    println!("Boson node {} is up and running.", node.id());

    println!("Waiting for the node to connect to the Boson network...");
    if tokio::time::timeout(Duration::from_secs(30), ready.notified()).await.is_err() {
        println!("Timed out waiting for a network connection; continuing anyway.");
    }

    let ap = match load_activeproxy_section(&opts.config) {
        Some(section) => {
            let options = match build_activeproxy_options(section, &data_dir, user_keypair) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error building activeproxy options: {e}");
                    exit(1);
                }
            };
            match ActiveProxy::new(node.clone(), options) {
                Ok(ap) => Some(Arc::new(ap)),
                Err(e) => {
                    eprintln!("Creating ActiveProxy client error: {e}");
                    exit(1);
                }
            }
        }
        None => None,
    };

    // `ProxyClient::start` drives its own single-threaded runtime and blocks
    // until stopped, so it needs its own OS thread.
    let ap_thread = ap.map(|ap| std::thread::spawn(move || {
        if let Err(e) = ap.start() {
            eprintln!("ActiveProxy client stopped with error: {e}");
        }
    }));

    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Failed to listen for shutdown signal.");
    }

    println!("Shutting down...");
    let _ = node.stop().await;
    if let Some(handle) = ap_thread {
        let _ = handle.join();
    }
}
