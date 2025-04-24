use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use std::process;
use clap::Parser;
use log::{info, debug, warn};
use rand::seq::SliceRandom;
use rand::thread_rng;

use boson::{
    Node,
    configuration as cfg,
    signature,
    Id,
    messaging::client,
    PeerInfo,
    NodeInfo,
    error::Result
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

    let mut peer: Option<PeerInfo> = None;
    let mut ni: Option<NodeInfo> = None;

    let peerid = messsaging_cfg.server_peerid().parse::<Id>().unwrap();

    _ = lookup_peer(node.clone(), &peerid, |v1, v2| {
        peer = v1.clone();
        ni = v2.clone();
    }).await.map_err(|e| {
        println!("error: {}", e);
        process::exit(-1);
    });

    let Some(peer) = peer else {
        eprintln!("Peer not found!!!");
        return;
    };

    let Some(ni) = ni else {
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

async fn lookup_peer<F>(node: Arc<Mutex<Node>>, peerid: &Id, mut cb: F) -> Result<()>
where
    F: FnMut(Option<PeerInfo>, Option<NodeInfo>),
{
    info!("MessagingClient is trying to find peer {} and its host node via DHT network...", peerid);

    let node = node.lock().unwrap();
    let mut peers = node.find_peer(peerid, Some(4), None).await.map_err(|e| {
        warn!("Trying to find peer but error: {}, please try it later!!!", e);
        e
    })?;

    if peers.is_empty() {
        warn!("No peers with peerid {} is found at this moment, please try it later!!!", peerid);
        cb(None, None);
        return Ok(());
    }

    debug!("Discovered {} satisfied peers, extracting each node's infomation...", peers.len());

    peers.shuffle(&mut thread_rng());
    while let Some(peer) = peers.pop() {
        let nodeid = peer.nodeid();
        debug!("Trying to lookup node {} hosting the peer {} ...", nodeid, peerid);

        let result = node.find_node(nodeid, None).await.map_err(|e| {
            warn!("Failed to find node {}, error: {}", nodeid, e);
            e
        })?;

        if result.is_empty() {
            warn!("can't locate node: {}! Go on next ...", nodeid);
            continue;
        }

        let mut ni = None;
        if let Some(v6) = result.v6() {
            ni = Some(v6.clone());
        }
        if let Some(v4) = result.v4() {
            ni = Some(v4.clone());
        }
        let Some(ni) = ni else {
            continue;
        };

        info!("Found peer {} and its host node {}.", peer.id(), node.id());
        cb(Some(peer), Some(ni));
        return Ok(())
    }

    cb(None, None);
    Ok(())
}
