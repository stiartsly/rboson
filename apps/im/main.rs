use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use clap::Parser;

use boson::{
    Node,
    configuration as cfg,
    signature,
    Id,
    messaging::client,
    appdata_store::AppDataStoreBuilder,
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
    let mut b = cfg::Builder::new();
    b.load(&opts.config)
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let cfg  = b.build().unwrap();
    let Some(user_cfg) = cfg.user() else {
        eprint!("User not found in configuration file");
        return;
    };

    let Some(messsaging_cfg) = cfg.messaging() else {
        eprint!("Messaging not found in configuration file");
        return;
    };

    let result = Node::new(&cfg);
    if let Err(e) = result {
        panic!("Creating Node instance error: {e}")
    }

    let node = Arc::new(Mutex::new(result.unwrap()));
    _ = node.lock()
        .unwrap()
        .start();

    thread::sleep(Duration::from_secs(2));
    let peerid = messsaging_cfg.server_peerid().parse::<Id>().unwrap();

    let mut path = String::new();
    path.push_str(cfg.storage_path());
    path.push_str("/messaging");

    let mut appdata_store = AppDataStoreBuilder::new("im")
        .with_path(path.as_str())
        .with_node(&node)
        .with_peerid(&peerid)
        .build()
        .unwrap();

    if let Err(e) = appdata_store.load().await {
        eprintln!("error: {e}");
        return;
    }

    let Some(peer) = appdata_store.service_peer() else {
        eprintln!("Peer not found!!!");
        return;
    };

    let Some(ni) = appdata_store.service_node() else {
        eprintln!("Node hosting the peer not found!!!");
        return;
    };

    println!("peer: {}", peer);
    println!("ni: {}", ni);

    let sk: signature::PrivateKey = match user_cfg.private_key().try_into() {
        Ok(key) => key,
        Err(_) => {
            eprint!("Failed to convert private key");
            return;
        }
    };

    let Some(messaing_cfg) = cfg.messaging() else {
        eprint!("Messaging not found in configuration file");
        return;
    };
    println!("Messaging peerid: {}", messaing_cfg.server_peerid());

    let Ok(client) = client::Builder::new()
        .with_user_name(user_cfg.name().map_or("test", |v|v))
        .with_user_key(signature::KeyPair::from(&sk))
        .with_node(node.clone())
        .with_device_key(signature::KeyPair::random())
        .with_deivce_name("test_device")
        .with_app_name("im_app")
        .register_user_and_device(user_cfg.password().map_or("password", |v|v))
        .with_peerid(peer.id())
        .with_nodeid(ni.id())
        .with_api_url(peer.alternative_url().as_ref().unwrap())
        .build().await else {
        eprint!("Failed to create client");
        return;
    };

    _ = client.start();
    client.stop();

    thread::sleep(Duration::from_secs(60*100));
    let _ = node.lock()
        .unwrap()
        .stop();
}
