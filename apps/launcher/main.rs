use clap::Parser;
use std::{env, process::exit, sync::Arc, time::Duration};
use tokio::sync::Notify;

use boson::activeproxy::{Client as ActiveProxyClient, Options as ActiveProxyOptions};
use boson::dht::{ConnectionStatus, ConnectionStatusListener, Node, NodeOptions};
use boson::director::{Client as DirectorClient, Options as DirectorOptions};
use boson::{
    errors::{ArgumentError, StateError},
    Id, Network, Result,
};

const DEFAULT_ACTIVEPROXY_CONFIG: &str = "apps/launcher/activeproxy.yaml";
const DEFAULT_NODE_CONFIG: &str = "apps/launcher/node.yaml";
//const DEFAULT_DIRECTOR_URL: &str = "https://47.101.142.224:9000";
const DEFAULT_INSECURE: bool = true;

#[derive(Parser, Debug)]
#[command(name = "launcher")]
#[command(version = "1.0")]
#[command(about = "Boson launcher service", long_about = None)]
struct Options {
    /// The DHT node configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// The ActiveProxy configuration file
    #[arg(long, value_name = "FILE")]
    activeproxy_config: Option<String>,

    /// Director service url
    #[arg(long, value_name = "URL")]
    director_url: Option<String>,
}

/// Notifies once the node has connected to the Boson network.
struct ReadyListener(Arc<Notify>);
impl ConnectionStatusListener for ReadyListener {
    fn status_changed(
        &self,
        network: Network,
        new_status: ConnectionStatus,
        old_status: ConnectionStatus,
    ) {
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

async fn ensure_device_admitted(
    director: &DirectorClient,
    user_id: &Id,
    device_id: &Id,
) -> Result<()> {
    if director.user_id() != Some(user_id) {
        return Err(ArgumentError::new(
            "Director and ActiveProxy configurations use different user identities",
        ));
    }
    if director.device_id() != Some(device_id) {
        return Err(ArgumentError::new(
            "Director and ActiveProxy configurations use different device identities",
        ));
    }

    let devices = director.list_devices().await?;

    if devices.iter().any(|device| device.id() == device_id) {
        println!("ActiveProxy device {device_id} is admitted by the Director.");
        return Ok(());
    }

    director
        .register_device("Launcher", "Boson Launcher", None)
        .await?;

    let admitted = director
        .list_devices()
        .await?
        .iter()
        .any(|device| device.id() == device_id);
    if !admitted {
        return Err(StateError::new(format!(
            "Director did not return the newly registered device {device_id}"
        )));
    }

    println!("Registered ActiveProxy device {device_id} with the Director.");
    Ok(())
}

fn build_director_options(
    director_url: impl AsRef<str>,
    activeproxy_options: &ActiveProxyOptions,
) -> Result<DirectorOptions> {
    let user_id = activeproxy_options
        .user_id()
        .cloned()
        .ok_or_else(|| ArgumentError::new("ActiveProxy configuration is missing userId"))?;
    let device_private_key = activeproxy_options
        .device_private_key()
        .cloned()
        .ok_or_else(|| {
            ArgumentError::new("ActiveProxy configuration is missing devicePrivateKey")
        })?;

    let mut options = DirectorOptions::new(director_url)?
        .with_user_id(user_id)
        .with_device_private_key(device_private_key)
        .with_insecure(DEFAULT_INSECURE);

    if let Some(user_private_key) = activeproxy_options.user_private_key().cloned() {
        options = options.with_user_private_key(user_private_key);
    }

    Ok(options)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let options = Options::parse();

    let ap_config = options
        .activeproxy_config
        .as_deref()
        .map(str::to_owned)
        .unwrap_or(format!("{DEFAULT_ACTIVEPROXY_CONFIG}"));

    let ap_opts = match ActiveProxyOptions::load(&ap_config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error building ActiveProxy Options: {e}");
            exit(1);
        }
    };
    if let Err(e) = ap_opts.check_completeness() {
        eprintln!("{e}");
        exit(1);
    }

    let director_url = options
        .director_url
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("BOSON_DIRECTOR_URL").ok());

    let Some(ref director_url) = director_url else {
        eprintln!("Director URL is not specified");
        exit(1);
    };

    let dir_opts = match build_director_options(&director_url, &ap_opts) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("Error building Director options: {e}");
            exit(1);
        }
    };

    let director = match DirectorClient::new(dir_opts) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Creating Director client error: {e}");
            exit(1);
        }
    };

    let user_id = match ap_opts.user_id() {
        Some(user_id) => user_id.clone(),
        None => {
            eprintln!("ActiveProxy configuration is missing userId");
            exit(1);
        }
    };
    let device_id = match ap_opts.device_id() {
        Some(device_id) => device_id.clone(),
        None => {
            eprintln!("ActiveProxy configuration is missing devicePrivateKey");
            exit(1);
        }
    };

    if let Err(e) = ensure_device_admitted(&director, &user_id, &device_id).await {
        eprintln!("Admitting the ActiveProxy device through the Director failed: {e}");
        exit(1);
    }

    let config = options
        .config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("NODE_CONFIG").ok())
        .unwrap_or_else(|| DEFAULT_NODE_CONFIG.to_string());

    let node_options = match NodeOptions::load(config) {
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
    if tokio::time::timeout(Duration::from_secs(30), ready.notified())
        .await
        .is_err()
    {
        println!("Timed out waiting for a network connection; continuing anyway.");
    }

    let ap = match ActiveProxyClient::new(Some(node.clone()), ap_opts) {
        Ok(ap) => ap,
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
