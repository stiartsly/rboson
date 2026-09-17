use std::{
    env,
    process::exit,
    sync::Arc,
    time::Duration,
};
use clap::Parser;
use tokio::sync::Notify;

use boson::{
    Id,
    Network,
    Result,
    dht::{
        Node,
        ConnectionStatus,
        ConnectionStatusListener,
        NodeOptions,
    },
    activeproxy::{
        Client as ActiveProxyClient,
        Options as ActiveProxyOptions,
    },
    director::{
        Client as DirectorClient,
        Options as DirectorOptions,
        NotFoundError,
        UnauthorizedError,
    },
    errors::{ArgumentError, StateError},
};

const DEFAULT_ACTIVEPROXY_CONFIG: &str = "apps/launcher/activeproxy.yaml";
const DEFAULT_NODE_CONFIG: &str = "apps/launcher/node.yaml";

const DEFAULT_DIRECTOR_URL: &str = "https://47.101.142.224:9000";
//const DEFAULT_DIRECTOR_NODEID: &str = "GhVW54uEd179PzRPpaiENKZuMezMNExTP6bXRK3rLDAQ";
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

    let devices = match director.list_devices().await {
        Ok(devices) => devices,
        Err(e)
            if e.downcast_ref::<UnauthorizedError>().is_some()
                || e.downcast_ref::<NotFoundError>().is_some() =>
        {
            director.register_user().await?;
            director.list_devices().await?
        }
        Err(e) => return Err(e),
    };

    if devices.iter().any(|device| device.id() == device_id) {
        println!("ActiveProxy device {device_id} is admitted by the Director.");
        return Ok(());
    }

    let registration = director.options().registration().ok_or_else(|| {
        StateError::new("Director registration settings are required to register the ActiveProxy device")
    })?;
    let name = registration.device_name().ok_or_else(|| {
        StateError::new("Director device.name is required to register the ActiveProxy device")
    })?;
    let app = registration.app_name().ok_or_else(|| {
        StateError::new("Director device.app is required to register the ActiveProxy device")
    })?;
    director
        .register_device(name, app, registration.passphrase())
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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let options = Options::parse();

    let ap_config = options
        .activeproxy_config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("ACTIVEPROXY_CONFIG").ok())
        .unwrap_or(DEFAULT_ACTIVEPROXY_CONFIG.to_string());

    let ap_opts = match ActiveProxyOptions::load(&ap_config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error building ActiveProxy configuration: {e}");
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
        .or_else(|| env::var("DIRECTOR_URL").ok())
        .unwrap_or(DEFAULT_DIRECTOR_URL.to_string());

    let dir_opts = DirectorOptions::new(director_url)
        .unwrap()
        .with_insecure(DEFAULT_INSECURE)
        .with_user_id(ap_opts.user_id().cloned().unwrap())
        //.with_user_private_key(ap_opts.user_private_key().cloned().unwrap())
        .with_device_private_key(ap_opts.device_private_key().cloned().unwrap());

    if let Err(e) = dir_opts.check_completeness() {
        eprintln!("{e}");
        exit(1);
    }

    let director = DirectorClient::new(dir_opts)
        .map_err(|e| {
            eprintln!("Creating Director client error: {e}");
            exit(1);
        })
        .unwrap();

    let user_id = ap_opts.user_id().cloned().unwrap();
    let device_id = ap_opts.device_id().cloned().unwrap();

    if let Err(e) = ensure_device_admitted(
        &director,
        &user_id,
        &device_id,
    ).await {
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
    if tokio::time::timeout(Duration::from_secs(30), ready.notified()).await.is_err() {
        println!("Timed out waiting for a network connection; continuing anyway.");
    }

    let ap = match ActiveProxyClient::new(Some(node.clone()), ap_opts) {
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
