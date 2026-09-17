use std::{
    env,
    fs,
    net::{IpAddr, SocketAddr},
    path::Path
};
use tokio::time::{sleep, Duration};
use get_if_addrs::get_if_addrs;
use boson::{
    Id,
    Node,
    NodeInfo,
    signature,
    dht::NodeOptions,
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

fn load_or_generate_key(path: &str) -> signature::PrivateKey {
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

fn parse_bootstrap(value: &str) -> Result<NodeInfo, String> {
    let (id, address) = value.split_once('@').ok_or_else(|| format!(
        "Invalid bootstrap '{value}': expected NODEID@IP:PORT"
    ))?;

    let id = Id::try_from(id).map_err(|e| format!(
        "Invalid bootstrap node ID '{id}': {e}"
    ))?;
    let address = address.parse::<SocketAddr>().map_err(|e| format!(
        "Invalid bootstrap address '{address}': {e}"
    ))?;

    Ok(NodeInfo::new(id, address))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut path = get_storage_path(".boson_sample");
    let mut port = 39010 as u16;
    let mut bootstrap_nodes = Vec::new();

    let host = match get_current_ip_address() {
        Some(addr) => addr,
        _ => return,
    }.to_string();

    let args: Vec<String> = env::args().collect();

    let mut iter = args.iter().skip(1);
    while let Some(argv) = iter.next() {
        match argv.as_str() {
            "--datadir" => {
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
            "--bootstrap" => {
                let Some(arg) = iter.next() else {
                    eprintln!("Missing value for --bootstrap; expected NODEID@IP:PORT");
                    return;
                };
                match parse_bootstrap(arg) {
                    Ok(node) => bootstrap_nodes.push(node),
                    Err(e) => {
                        eprintln!("{e}");
                        return;
                    }
                }
            }
            arg if arg.starts_with("--bootstrap=") => {
                match parse_bootstrap(&arg["--bootstrap=".len()..]) {
                    Ok(node) => bootstrap_nodes.push(node),
                    Err(e) => {
                        eprintln!("{e}");
                        return;
                    }
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", argv);
                return;
            },
        }
    };

    let private_key = load_or_generate_key(&path);
    let options = NodeOptions::new(private_key)
        .with_port(port)
        .with_host4(host.as_str())
        .with_data_dir(path.as_str())
        .with_log_level(log::LevelFilter::Debug)
        .with_log_console(true)
        .with_bootstrap_nodes(bootstrap_nodes);

    let node = Node::new(options).unwrap();
    let _ = node.start().await;

    println!("The sample node is running on {}:{}", host, port);
    sleep(Duration::from_secs(60*10)).await;
    let _ = node.stop().await;
}
