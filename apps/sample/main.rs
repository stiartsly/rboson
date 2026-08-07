use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::Path;
use tokio::time::{sleep, Duration};
use get_if_addrs::get_if_addrs;

use boson::{
    Node,
    NodeConfig,
    signature,
    cfg::configuration as cfg,
};

fn get_storage_path(input: &str) -> String {
    let path = env::current_dir().unwrap().join(input);

    if !fs::metadata(&path).is_ok() {
        match fs::create_dir(&path) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create directory: {}", e);
            }
        }
    }
    path.display().to_string()
}

fn get_current_ip_address() -> Option<IpAddr>{
    match get_if_addrs() {
        Ok(if_addrs) => {
            for iface in if_addrs {
                let ip_addr = iface.ip();
                if ip_addr.is_ipv4() && !ip_addr.is_loopback() {
                    return Some(ip_addr);
                }
            }
            panic!("No active local IP address!!!");
        }
        Err(e) => {
            panic!("Failed to fetch local IP address: {}", e);
        },
    }
}

// Reuses the node's identity key across restarts by caching it under the storage path.
fn load_or_create_key(path: &str) -> signature::PrivateKey {
    let key_path = Path::new(path).join("key");
    if let Ok(hex) = fs::read_to_string(&key_path) {
        if let Ok(key) = signature::PrivateKey::try_from(hex.trim()) {
            return key;
        }
    }

    let key = signature::KeyPair::random().private_key().clone();
    fs::write(&key_path, key.to_hexstr())
        .unwrap_or_else(|e| panic!("Failed to save node key: {}", e));
    key
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut path = get_storage_path(".target_data");
    let mut port = 39001 as u16;

    let ip_str = {
        match get_current_ip_address() {
            Some(addr) => addr,
            None => return
        }.to_string()
    };

    let args: Vec<String> = env::args().collect();

    let mut iter = args.iter();
    while let Some(argv) = iter.next() {
        match argv.as_ref() {
            "--storepath" => {
                if let Some(arg) = iter.next() {
                    path = arg.clone();
                }
            }
            "--port" =>  {
                if let Some(arg) = iter.next() {
                    if let Ok(val) = arg.parse::<u16>() {
                        port = val;
                    }
                }
            }
            _ => {},
        }
    };

    let private_key = load_or_create_key(&path);

    let node_cfg = cfg::Builder::new()
        .with_port(port)
        .with_host4(&ip_str)
        .with_data_dir(path.as_str())
        .with_private_key(private_key)
        .with_database_uri("jdbc:sqlite:node.db")
        .build()
        .unwrap();

    node_cfg.dump();

    let node = Node::new(Box::new(node_cfg)).unwrap();
    let _ = node.start().await;

    println!("Target node running on {}:{} (storage: {})", ip_str, port, path);
    sleep(Duration::from_secs(60*100)).await;
    let _ = node.stop().await;
}

