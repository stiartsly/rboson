use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tokio::sync::Notify;

use boson::{
    Network,
    dht::{
        Node,
        ConnectionStatus,
        ConnectionStatusListener,
        NodeOptionsBuilder,
    },
    activeproxy::{
        ActiveProxyClient as ActiveProxy,
        OptionsBuilder as ActiveProxyOptionsBuilder,
    },
};

#[derive(Parser, Debug)]
#[command(name = "launcher")]
#[command(version = "1.0")]
#[command(about = "Boson launcher service", long_about = None)]
struct Options {
    /// The DHT node configuration file
    #[arg(short, long, value_name = "FILE", default_value = "node.yaml")]
    config: String,

    /// The ActiveProxy configuration file
    #[arg(long, value_name = "FILE", default_value = "activeproxy.yaml")]
    activeproxy_config: String,
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

    let node_options = match NodeOptionsBuilder::new()
        .load_from(&opts.config)
        .and_then(NodeOptionsBuilder::build)
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error building node configuration: {e}");
            exit(1);
        }
    };

    let node = match Node::new(node_options) {
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

    let activeproxy_options = match ActiveProxyOptionsBuilder::load_from(&opts.activeproxy_config)
        .and_then(ActiveProxyOptionsBuilder::build)
    {
        Ok(options) => options,
        Err(e) => {
            eprintln!("Error building ActiveProxy configuration: {e}");
            exit(1);
        }
    };
    let ap = match ActiveProxy::new(Some(node.clone()), activeproxy_options) {
        Ok(ap) => Arc::new(ap),
        Err(e) => {
            eprintln!("Creating ActiveProxy client error: {e}");
            exit(1);
        }
    };

    if let Err(e) = ap.start().await {
        eprintln!("ActiveProxy client stopped with error: {e}");
    }

    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Failed to listen for shutdown signal.");
    }

    println!("Shutting down...");

    let _ = ap.stop().await;

    let _ = node.stop().await;
}
