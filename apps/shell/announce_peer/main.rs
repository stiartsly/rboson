use std::sync::{Arc, Mutex};
use tokio::{
    sync::Notify,
    time::{sleep, timeout, Duration},
};
use clap::Parser;

use boson::{
    PeerInfo,
    Network,
    signature::{PrivateKey, KeyPair},
    dht::{
        Node,
        NodeConfig,
        NodeConfiguration,
        ConnectionStatus,
        ConnectionStatusListener,
    },
};

#[derive(Parser, Debug)]
#[command(about = "Announce Peer Shell", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    #[arg(short='P', long)]
    announce_peer: bool,
}

#[derive(Default)]
struct DefaultConnectionStatusListener {
    ready: Option<Arc<Notify>>,
    status: Option<Arc<Mutex<ConnectionStatus>>>,
}

impl ConnectionStatusListener for DefaultConnectionStatusListener {
    fn status_changed(&self,
        network: Network,
        new_status: ConnectionStatus,
        old_status: ConnectionStatus,
    ) {
        println!("\x1b[34mConnection status changed for network {}: {}->{}\x1b[0m", network, old_status, new_status);
        if let Some(status) = self.status.as_ref() {
            *status.lock().unwrap() = new_status;
        }
    }
    fn connecting(&self, network: Network) {
        println!("\x1b[34mConnecting to network {}...\x1b[0m", network);
    }
    fn connected(&self, network: Network) {
        println!("\x1b[34mConnected to network {}.\x1b[0m", network);
        if let Some(ready) = self.ready.as_ref() {
            ready.notify_one();
        }
    }
    fn disconnected(&self, network: Network) {
        println!("\x1b[34mDisconnected from network {}.\x1b[0m", network);
    }
}

// PEERID = 88WmYVuFpR1B6L5SJ9k1Xx9jkz225iWQ8qCNoNrrWdvz
const DEFAULT_ENDPOINT: &str = "www.example.com";
const DEFAULT_KEY: &str = "0x9bd52d532edf8b49c741285eba396946983018c07a40710892746d744861f94369ee886d1b79f802883d4188de80567c6bf5f687bc9b0b0dad19bbc2e54280c3";

fn generate_peerinfo(endpoint: &str, sk: Option<&str>) -> Result<PeerInfo, String> {
    let keypair = match sk {
        Some(v) => match PrivateKey::try_from(v) {
            Ok(sk) => KeyPair::from(sk),
            Err(e) => {
                println!("Invalid private key: {e}");
                return Err(e.to_string());
            }
        },
        _ => {
            let sk = PrivateKey::try_from(DEFAULT_KEY)
                .map_err(|e| format!("Invalid default key: {e}"))?;
            KeyPair::from(sk)
        }
    };

    let peer = match PeerInfo::builder(endpoint).with_key(keypair).build() {
        Ok(p) => p,
        Err(e) => {
            println!("Building peer info failed: {e}");
            return Err(e.to_string());
        }
    };
    Ok(peer)
}

async fn announce_peer(node: &Node, peer: &PeerInfo) -> bool {
    match node.announce_peer(&peer, -1, true).await {
        Ok(_) => {
            println!("\x1b[34mPeer announced successfully.\x1b[0m");
            true
        },
        Err(e) => {
            println!("\x1b[31mFailed to announce peer: {}\x1b[0m", e);
            false
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opts = Options::parse();
    let config = NodeConfiguration::load(
        opts.config.as_deref().unwrap_or("config.yaml")
    ).unwrap();

    #[cfg(feature = "inspect")] {
        config.dump();
    }

    let ready = Arc::new(Notify::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Disconnected));
    let listener = DefaultConnectionStatusListener {
        ready: Some(ready.clone()),
        status: Some(status.clone()),
    };

    let node = Node::new(Box::new(config)).unwrap();
    node.add_listener(listener);
    let _ = node.start().await;

    // Wait for the node to connect before announcing.
    println!("Waiting for the node to connect to the Boson network...");
    if let Err(_) = timeout(Duration::from_secs(60), ready.notified()).await {
        println!("Timeout waiting for connection; continuing anyway...");
    }

    let result = generate_peerinfo(DEFAULT_ENDPOINT, None);
    let peer = match result {
        Ok(p) => p,
        Err(e) => {
            println!("Failed to generate peer info: {e}");
            return;
        }
    };

    while opts.announce_peer {
        println!("Announcing peer {} with endpoint '{}' ...", peer.id(), peer.endpoint());
        let result = announce_peer(&node, &peer).await;
        if !result {
            let _ = node.stop().await;
            return;
        }
        println!("Waiting for 20 seconds before announcing again...");
        sleep(Duration::from_secs(20)).await;
    }

    let _ = node.stop().await;
}
