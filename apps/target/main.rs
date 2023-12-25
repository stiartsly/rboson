use std::env;
use std::fs;
use std::thread;
use std::net::IpAddr;
use tokio::time::Duration;
use get_if_addrs::get_if_addrs;

use boson::{
    Node,
    default_configuration as cfg
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

fn main() {
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

    let cfg = cfg::Builder::new()
        .with_listening_port(port)
        .with_ipv4(&ip_str)
        .with_storage_path(path.as_str())
        .build()
        .unwrap();

    let node = Node::new(cfg).unwrap();
    let _ = node.start();

    thread::sleep(Duration::from_secs(60*100));
    node.stop();
}
