use std::sync::{Arc, Mutex};
use tokio::{
    sync::Notify,
    time::{sleep, timeout, Duration},
};
use clap::Parser;

use boson::{
    Value,
    SignedBuilder,
    Network,
    NodeConfig,
    cfg::configuration,
    signature::{KeyPair, PrivateKey},
    dht::{
        Node,
        ConnectionStatus,
        ConnectionStatusListener,
    },
};

#[derive(Parser, Debug)]
#[command(about = "Announce Value Shell", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    #[arg(short='V', long)]
    announce_value: bool,
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

// VALUEID = 6tLSgsSng6m8LCv1VWFxsJT3SW5RWfjMqEAUNS52TsSm
const DEFAULT_VALUE_DATA: &str = "myvalue";
const DEFAULT_KEY: &str = "0x9bd52d532edf8b49c741285eba396946983018c07a40710892746d744861f94369ee886d1b79f802883d4188de80567c6bf5f687bc9b0b0dad19bbc2e54280c3";

fn generate_value(data: &str, seq: i32, sk: Option<&str>) -> Result<Value, String> {
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

    let mut builder = SignedBuilder::new(data.as_bytes());
    builder.with_keypair(&keypair);
    builder.with_sequence_number(seq);
    let value = match builder.build() {
        Ok(v) => v,
        Err(e) => {
            println!("Building value failed: {e}");
            return Err(e.to_string());
        }
    };
    Ok(value)
}

async fn announce_value(node: &Node, value: &Value) -> bool{
    println!("Announcing value {} ({} bytes) ...", value.id(), value.data().len());
    match node.store_value(value, -1, true).await {
        Ok(_) => {
            println!("\x1b[34mValue announced successfully.\x1b[0m");
            true
        },
        Err(e) => {
            println!("\x1b[31mFailed to announce value: {}\x1b[0m", e);
            false
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opts = Options::parse();
    let config = match configuration::Builder::new()
            .load(opts.config.as_deref().unwrap_or("config.yaml"))
            .and_then(|b| b.build())
    {
        Ok(v) => v,
        Err(e) => {
            println!("Loading configuration failed: {e}");
            return;
        }
    };

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

    let result = generate_value(DEFAULT_VALUE_DATA, 0, None);
    let value = match result {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to generate value: {e}");
            return;
        }
    };

    while opts.announce_value {
        println!("Announcing peer {}  ...", value.id());
        let result = announce_value(&node, &value).await;
        if !result {
            let _ = node.stop().await;
            return;
        }
        println!("Waiting for 20 seconds before announcing again...");
        sleep(Duration::from_secs(20)).await;
    }

    sleep(Duration::from_secs(5)).await;
    let _ = node.stop().await;
}
