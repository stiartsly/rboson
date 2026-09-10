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
    let mut path = get_storage_path(".sample_node");
    let mut port = 39010 as u16;
    let mut bootstrap_nodes = Vec::new();

    let ip_str = match get_current_ip_address() {
        Some(addr) => addr,
        _ => return,
    }.to_string();

    let args: Vec<String> = env::args().collect();

    let mut iter = args.iter().skip(1);
    while let Some(argv) = iter.next() {
        match argv.as_str() {
            "--store" => {
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
    let mut builder = cfg::Configuration::new();
    builder
        .with_port(port)
        .with_host4(&ip_str)
        .with_data_dir(path.as_str())
        .with_private_key(private_key)
        .with_log_level(log::LevelFilter::Debug)
        .with_database_uri("jdbc:sqlite:node.db");

    for bootstrap_node in bootstrap_nodes {
        builder.add_bootstrap_node(bootstrap_node);
    }
    let cfg = builder.build().unwrap();

    cfg.dump();

    let options = cfg.build_node_options().unwrap();
    let node = Node::new(options).unwrap();
    let _ = node.start().await;

    println!("Target node running on {}:{} (storage: {})", ip_str, port, path);
    sleep(Duration::from_secs(60*10)).await;
    let _ = node.stop().await;
}

#[cfg(test)]
mod tests {
    use super::parse_bootstrap;

    const NODE_ID: &str = "FyHfVWtscJWUeejGQaJXyUnjUcKFGSVVYmozqBuJSmjo";

    #[test]
    fn parses_bootstrap_node() {
        let node = parse_bootstrap(&format!("{NODE_ID}@155.138.245.211:39001")).unwrap();

        assert_eq!(node.id().to_string(), NODE_ID);
        assert_eq!(node.address().to_string(), "155.138.245.211:39001");
    }
}

// Here is the command to run the sample node with a bootstrap node:
// cargo r --bin node -- --bootstrap FyHfVWtscJWUeejGQaJXyUnjUcKFGSVVYmozqBuJSmjo@155.138.245.211:39001
