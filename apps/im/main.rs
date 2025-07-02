use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use clap::Parser;

use boson::{
    configuration as cfg,
    signature,
    Id,
    dht::Node,
    appdata_store::AppDataStoreBuilder,
    messaging,
};

#[derive(Parser, Debug)]
#[command(name = "Messaging")]
#[command(version = "1.0")]
#[command(about = "Boson Messaging", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: String,

    /// Run this program in daemon mode
    #[arg(short='D', long)]
    daemonize: bool
}

#[tokio::main]
async fn main() {
    let opts = Options::parse();
    let cfg = cfg::Builder::new()
        .load(&opts.config)
        .map_err(|e| panic!("{e}"))
        .unwrap()
        .build()
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let Some(ucfg) = cfg.user() else {
        eprintln!("User item is not found in config file");
        return;
    };

    let Some(mcfg) = cfg.messaging() else {
        eprintln!("Messaging item not found in config file");
        return;
    };

    let peerid = Id::try_from(mcfg.server_peerid())
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let result = Node::new(&cfg);
    if let Err(e) = result {
        eprintln!("Creating boson Node instance error: {e}");
        return;
    }

    let node = Arc::new(Mutex::new(result.unwrap()));
    node.lock().unwrap().start();

    thread::sleep(Duration::from_secs(1));

    let mut path = String::new();
    path.push_str(cfg.data_dir());
    path.push_str("/messaging");

    let mut appdata_store = AppDataStoreBuilder::new("im")
        .with_path(path.as_str())
        .with_node(&node)
        .with_peerid(&peerid)
        .build()
        .unwrap();

    if let Err(e) = appdata_store.load().await {
        eprintln!("Loading app data store error: {e}");
        node.lock().unwrap().stop();
        return;
    }

    let Some(peer) = appdata_store.service_peer() else {
        println!("Messaging peer is not found!!!, please run it later.");
        node.lock().unwrap().stop();
        return;
    };

    let Some(ni) = appdata_store.service_node() else {
        eprintln!("Node hosting the peer not found!!!");
        node.lock().unwrap().stop();
        return;
    };

    println!("Messaging Peer: {}", peer);
    println!("Messaging Node: {}", ni);

    let sk: signature::PrivateKey = match ucfg.private_key().try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Failed to convert private key from hex format");
            node.lock().unwrap().stop();
            return;
        }
    };

    let user_key = signature::KeyPair::from(&sk);
    let device_key = signature::KeyPair::random();

    let result = messaging::ClientBuilder::new()
        .with_user_name(ucfg.name().unwrap_or("guest"))
        .with_user_key(&user_key)
        .with_device_node(node.clone())
        .with_device_key(&device_key)
        .with_device_name("testing")
        .with_app_name("im")
        .with_messaging_repository(path.as_str())
        .register_user_and_device(ucfg.password().map_or("secret", |v|v))
        .with_peerid(peer.id())
        .with_nodeid(ni.id())
        .with_api_url(peer.alternative_url().as_ref().unwrap())
        .build()
        .await;

    let mut client = match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Creating messaging client instance error: {e}");
            node.lock().unwrap().stop();
            return;
        }
    };

    _ = client.start();
    thread::sleep(Duration::from_secs(1));
    _ = client.stop();
    node.lock().unwrap().stop();
}
