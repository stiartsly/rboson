use boson::{
    dht::NodeOptions,
    signature, Id, Node, NodeInfo,
};
use get_if_addrs::get_if_addrs;
use std::{
    env, fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};
use tokio::time::{sleep, Duration};

const DEFAULT_DATA_DIR: &str = ".sample_node";
const DEFAULT_PORT: u16 = 39088;
const DEFAULT_RUN_SECONDS: u64 = 60 * 10;

struct SampleConfig {
    data_dir: PathBuf,
    port: u16,
    bootstrap_node: Option<NodeInfo>,
    verbose: bool,
}

impl Default for SampleConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(DEFAULT_DATA_DIR),
            port: DEFAULT_PORT,
            bootstrap_node: None,
            verbose: false,
        }
    }
}

fn print_usage(program: &str) {
    println!(
        "\
Usage:
  {program} [--datadir DIR] [--port PORT] [--bootstrap NODEID@IP:PORT]

Options:
  --datadir DIR              Directory used to keep the sample node key and DHT database.
                             Default: {DEFAULT_DATA_DIR}
  --port PORT                UDP port used by the DHT node.
                             Default: {DEFAULT_PORT}
  --bootstrap NODEID@IP:PORT Add an existing DHT node as an entry point.
                             Can be repeated.
  --verbose                  Enable verbose logging (TRACE level).
  -h, --help                 Print this help.

Example:
  cargo run --bin anode -- \\
    --datadir .sample_node \\
    --port 39088 \\
    --bootstrap FyHfVWtscJWUeejGQaJXyUnjUcKFGSVVYmozqBuJSmjo@155.138.245.211:39001
"
    );
}

fn get_current_ip_address() -> Option<IpAddr> {
    let if_addrs = get_if_addrs().unwrap_or_else(|e| {
        panic!("Failed to fetch local IP address: {}", e);
    });

    for iface in if_addrs {
        let ip_addr = iface.ip();
        if ip_addr.is_ipv4() && !ip_addr.is_loopback() {
            return Some(ip_addr);
        }
    }
    None
}

fn ensure_data_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|e| format!("Failed to create data directory {}: {e}", path.display()))
}

fn load_or_generate_key(path: &Path) -> Result<signature::PrivateKey, String> {
    ensure_data_dir(path)?;

    let key_path = path.join("key");
    if let Ok(hex) = fs::read_to_string(&key_path) {
        if let Ok(key) = signature::PrivateKey::try_from(hex.trim()) {
            return Ok(key);
        }

        eprintln!(
            "Ignoring invalid existing key file {}; generating a new key.",
            key_path.display()
        );
    }

    let key = signature::KeyPair::random().private_key().clone();
    fs::write(&key_path, key.to_hexstr())
        .map_err(|e| format!("Failed to save node key {}: {e}", key_path.display()))?;
    Ok(key)
}

fn parse_bootstrap(value: &str) -> Result<NodeInfo, String> {
    let (id, address) = value
        .split_once('@')
        .ok_or_else(|| format!("Invalid bootstrap '{value}': expected NODEID@IP:PORT"))?;

    let id = Id::try_from(id).map_err(|e| format!("Invalid bootstrap node ID '{id}': {e}"))?;
    let address = address
        .parse::<SocketAddr>()
        .map_err(|e| format!("Invalid bootstrap address '{address}': {e}"))?;

    let ni = NodeInfo::new(id, address);
    println!("{}", ni);
    // Ok(NodeInfo::new(id, address))
    Ok(ni)
}

fn parse_args() -> Result<Option<SampleConfig>, String> {
    let mut config = SampleConfig::default();
    let args = env::args().collect::<Vec<_>>();
    let mut iter = args.iter().skip(1);

    while let Some(argv) = iter.next() {
        match argv.as_str() {
            "-h" | "--help" => {
                print_usage(args.first().map(String::as_str).unwrap_or("anode"));
                return Ok(None);
            }
            "--datadir" => {
                let Some(arg) = iter.next() else {
                    return Err("Missing --datadir value".to_string());
                };
                config.data_dir = PathBuf::from(arg);
            }
            "--port" => {
                let Some(arg) = iter.next() else {
                    return Err("Missing --port value".to_string());
                };
                config.port = arg
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid --port value '{arg}': {e}"))?;
            }
            "--bootstrap" => {
                let Some(arg) = iter.next() else {
                    return Err("Missing --bootstrap value; expected NODEID@IP:PORT".to_string());
                };
                config.bootstrap_node = Some(parse_bootstrap(arg)?);
            }
            "--verbose" => {
                config.verbose = true;
            }
            arg if arg.starts_with("--bootstrap=") => {
                config.bootstrap_node = Some(parse_bootstrap(&arg["--bootstrap=".len()..])?);
            }
            _ => {
                return Err(format!("Unknown argument: {argv}"));
            }
        }
    }

    Ok(Some(config))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let Some(config) = (match parse_args() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("Run with --help to see usage.");
            return;
        }
    }) else {
        return;
    };

    let Some(host) = get_current_ip_address() else {
        eprintln!("No non-loopback IPv4 address was found.");
        eprintln!("Connect to a network interface or replace get_current_ip_address() for local testing.");
        return;
    };
    let host = host.to_string();

    // A DHT node identity is a long-term Ed25519 signing key. This sample stores
    // it under the data directory so repeated runs keep the same node ID.
    let private_key = match load_or_generate_key(&config.data_dir) {
        Ok(key) => key,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    // NodeOptions is the main SDK configuration object for creating a DHT node.
    // At minimum, a node needs an identity key, a listen host/port, and a data
    // directory. Bootstrap nodes are optional, but needed to join an existing network.
    let data_dir = config.data_dir.to_string_lossy();
    let log_level = if config.verbose {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    };
    let mut bootstrap_nodes: Vec<NodeInfo> = Vec::new();
    if let Some(ref bootstrap_node) = config.bootstrap_node {
        bootstrap_nodes.push(bootstrap_node.clone());
    }
    let options = NodeOptions::new(private_key)
        .with_port(config.port)
        .with_host4(host.as_str())
        .with_data_dir(data_dir.as_ref())
        .with_log_level(log_level)
        .with_bootstrap_nodes(bootstrap_nodes);

    let node = match Node::new(options) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Creating DHT node failed: {e}");
            return;
        }
    };

    if let Err(e) = node.start().await {
        eprintln!("Starting DHT node failed: {e}");
        return;
    }

    println!("The sample DHT node is running.");
    println!("  node id : {}", node.id());
    println!("  address : {}:{}", host, config.port);
    println!("  data dir: {}", config.data_dir.display());
    if let Some(ref ni) = config.bootstrap_node {
        println!("  bootstrap node: {}", ni);
    }
    println!("The process will stop automatically after {DEFAULT_RUN_SECONDS} seconds.");

    sleep(Duration::from_secs(DEFAULT_RUN_SECONDS)).await;

    if let Err(e) = node.stop().await {
        eprintln!("Stopping DHT node failed: {e}");
    }
}
