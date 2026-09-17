use std::{
    process::exit,
    sync::Arc,
    time::Duration,
};
use std::env;

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
        Client as ActiveProxy,
        Options as ActiveProxyOptions,
    },
    director::{
        DirectorClient,
        DirectorOptions,
        NotFoundError,
        UnauthorizedError,
    },
    errors::{ArgumentError, StateError},
};

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

    /// The Director configuration used to admit the ActiveProxy device
    #[arg(long, value_name = "FILE")]
    director_config: Option<String>,
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

    let registration = director.options().registration();
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
    let opts = Options::parse();

    let activeproxy_config = opts
        .activeproxy_config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("ACTIVEPROXY_CONFIG").ok())
        .unwrap_or_else(|| "apps/launcher/activeproxy.yaml".to_string());
    let activeproxy_options = match ActiveProxyOptions::load(&activeproxy_config) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("Error building ActiveProxy configuration: {e}");
            exit(1);
        }
    };

    let director_config = opts
        .director_config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("DIRECTOR_CONFIG").ok())
        .unwrap_or_else(|| "apps/launcher/director.yaml".to_string());
    let director_options = match DirectorOptions::load(&director_config) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("Error building Director configuration: {e}");
            exit(1);
        }
    };
    let director = match DirectorClient::new(director_options) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Creating Director client failed: {e}");
            exit(1);
        }
    };

    let activeproxy_userid = activeproxy_options.user_id().clone();
    let activeproxy_device_key = activeproxy_options.device_key();

    let activeproxy_device_id = Id::from(activeproxy_device_key.public_key());
    if let Err(e) = ensure_device_admitted(
        &director,
        &activeproxy_userid,
        &activeproxy_device_id,
    ).await {
        eprintln!("Admitting the ActiveProxy device through the Director failed: {e}");
        exit(1);
    }

    let config = opts
        .config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("NODE_CONFIG").ok())
        .unwrap_or_else(|| "apps/launcher/node.yaml".to_string());

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
