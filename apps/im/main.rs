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
};

#[derive(Parser, Debug)]
#[command(name = "Messaging")]
#[command(version = "1.0")]
#[command(about = "Boson Messaging", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: String,

    /// IPv4 address used for listening.
    #[arg(short = '4', long, value_name = "IPv4")]
    addr4: Option<String>,

    /// IPv6 address used for listening.
    #[arg(short = '6', long, value_name = "IPv6")]
    addr6: Option<String>,

    /// The directory for storing node data
    #[arg(short, long, value_name = "PATH")]
    storage: Option<String>,

    /// The port used for listening
    #[arg(short, long, default_value_t = 39011)]
    port: u16,

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

    if let Some(path) = opts.storage.as_ref() {
        b.with_storage_path(path);
    }

    let cfg  = b.build().unwrap();
    let Some(user_cfg) = cfg.user() else {
        eprint!("User not found in configuration file");
        return;
    };

    user_cfg.name().as_ref().map(|v| {
        println!("User name: {v}");
    });

    user_cfg.password().as_ref().map(|v| {
        println!("User password: {v}");
    });

    let result = Node::new(&cfg);
    if let Err(e) = result {
        panic!("Creating Node instance error: {e}")
    }

    let node = Arc::new(Mutex::new(result.unwrap()));
    _ = node.lock()
        .unwrap()
        .start();

    thread::sleep(Duration::from_secs(2));

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

    let peerid = Id::random();
    let nodeid = Id::random();

    let Ok(client) = client::Builder::new()
        .with_user_name(user_cfg.name().map_or("test", |v|v))
        .with_user_key(signature::KeyPair::from(&sk))
        .with_node(node.clone())
        .with_device_key(signature::KeyPair::random())
        .with_deivce_name("test_device")
        .with_app_name("im_app")
        .register_user_and_device(user_cfg.password().map_or("password", |v|v))
        .with_peerid(peerid)
        .with_nodeid(nodeid)
        .with_api_url("https://www.example.com")
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
