use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use clap::Parser;

use boson::{
    dht::Node,
    configuration as cfg,
    ActiveProxyClient as ActiveProxy,
};

#[derive(Parser, Debug)]
#[command(name = "Laucnher")]
#[command(version = "1.0")]
#[command(about = "Boson launcher service", long_about = None)]
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

fn main() {
    let opts = Options::parse();
    let mut b = cfg::Builder::new();
    b.load(&opts.config)
        .map_err(|e| panic!("{e}"))
        .unwrap();

    if let Some(path) = opts.storage.as_ref() {
        b.with_storage_path(path);
    }

    let cfg  = b.build().unwrap();
    let result = Node::new(&cfg);
    if let Err(e) = result {
        panic!("Creating Node instance error: {e}")
    }

    let node = Arc::new(Mutex::new(result.unwrap()));
    let _ = node.lock()
        .unwrap()
        .start();

    thread::sleep(Duration::from_secs(2));

    let result = ActiveProxy::new(node.clone(), &cfg);
    if let Err(e) = result {
        panic!("Creating ActiveProxy client error: {e}")
    }

    let ap = result.unwrap();
    match ap.start() {
        Ok(_) => {},
        Err(e) => panic!("{e}")
    }

    thread::sleep(Duration::from_secs(60*100));
    let _ = node.lock()
        .unwrap()
        .stop();
}
