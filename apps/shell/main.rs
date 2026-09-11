use clap::{arg, value_parser, ArgMatches, Command, Parser};
use reedline::{ExternalPrinter, Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal};
use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use boson::{
    cfg::configuration,
    core::logger,
    dht::{ConnectionStatus, ConnectionStatusListener, Node},
    signature::{KeyPair, PrivateKey},
    Id, Network,
};
use log::{debug, info, warn};

mod announce_peer;
mod announce_value;

struct ConnectionReadiness {
    connected_networks: AtomicU8,
    changed: tokio::sync::Notify,
}

impl ConnectionReadiness {
    fn new() -> Self {
        Self {
            connected_networks: AtomicU8::new(0),
            changed: tokio::sync::Notify::new(),
        }
    }

    fn is_connected(&self) -> bool {
        self.connected_networks.load(Ordering::Acquire) != 0
    }

    fn set_network_connected(&self, network: Network, connected: bool) {
        let network_mask = match network {
            Network::IPv4 => 0b01,
            Network::IPv6 => 0b10,
        };
        if connected {
            self.connected_networks
                .fetch_or(network_mask, Ordering::AcqRel);
        } else {
            self.connected_networks
                .fetch_and(!network_mask, Ordering::AcqRel);
        }
        self.changed.notify_waiters();
    }

    async fn wait_until_connected(&self) {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_connected() {
                return;
            }
            notified.await;
        }
    }
}

struct DefaultConnectionStatusListener {
    readiness: Arc<ConnectionReadiness>,
}

impl ConnectionStatusListener for DefaultConnectionStatusListener {
    fn status_changed(
        &self,
        network: Network,
        new_status: ConnectionStatus,
        old_status: ConnectionStatus,
    ) {
        debug!("Connection status changed for network {network}: {old_status}->{new_status}");
    }
    fn connecting(&self, network: Network) {
        info!("Connecting to network {network}...");
    }
    fn connected(&self, network: Network) {
        info!("Connected to network {network}.");
        self.readiness.set_network_connected(network, true);
    }
    fn disconnected(&self, network: Network) {
        warn!("Disconnected from network {network}.");
        self.readiness.set_network_connected(network, false);
    }
}

struct ShellPrompt;
impl Prompt for ShellPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        "boson> ".into()
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        "".into()
    }
    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        "".into()
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        "... ".into()
    }
    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<'_, str> {
        "".into()
    }
}

#[derive(Parser, Debug)]
#[command(about = "Boson Shell", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// The data directory
    #[arg(short, long, value_name = "PATH")]
    datadir: Option<String>,

    /// The private key
    #[arg(short = 'k', long, value_name = "STRING")]
    privatekey: Option<String>,

    /// The port to listen on
    #[arg(short = 'p', long, value_name = "PORT")]
    port: Option<u16>,

    /// Enable log output on the console
    #[arg(long)]
    log: bool,
}

/// Builds the interactive shell's subcommand tree.
///
/// Sub commands:
///   - `announcepeer [ENDPOINT] [-k/--key <PRIVATE_KEY>]`
///   - `announcevalue <VALUE>`
///   - `findnode <ID>`
///   - `findpeer <ID> [-c/--count <COUNT>]`
///   - `findvalue <ID>`
///   - `identity`
///   - `log [on|off]`
///   - `status`
fn build_cli() -> Command {
    Command::new("boson")
        .no_binary_name(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .subcommand(
            Command::new("announcepeer")
                .visible_alias("announce_peer")
                .about("Announce a peer to the Boson network")
                .arg(arg!([ENDPOINT] "Endpoint value for the announced peer")
                    .default_value(announce_peer::DEFAULT_ENDPOINT))
                .arg(arg!(-k --key <PRIVATE_KEY> "Private key (hex or base58) for the peer identity; \
                    defaults to this node's own key")
                    .required(false))
        )
        .subcommand(
            Command::new("announcevalue")
                .visible_alias("announce_value")
                .about("Announce an immutable value to the Boson network")
                .arg(arg!(<VALUE> "Value data (string) to announce"))
        )
        .subcommand(
            Command::new("findnode")
                .visible_alias("find_node")
                .about("Look up a node by id")
                .arg(arg!(<ID> "Target node id (base58)"))
        )
        .subcommand(
            Command::new("findpeer")
                .visible_alias("find_peer")
                .about("Look up peers announced under an id")
                .arg(arg!(<ID> "Target peer id (base58)"))
                .arg(arg!(-c --count <COUNT> "Expected number of peers")
                    .default_value("8")
                    .value_parser(value_parser!(usize)))
        )
        .subcommand(
            Command::new("findvalue")
                .visible_alias("find_value")
                .about("Look up a value by id")
                .arg(arg!(<ID> "Target value id (base58)"))
        )
        .subcommand(
            Command::new("keygen")
                .about("Generate a random key identity")
        )
        .subcommand(
            Command::new("me")
                .about("Show my key identity")
        )
        .subcommand(
            Command::new("log")
                .about("Enable or disable console log output; logs are always written to the file")
                .arg(
                    arg!([STATE] "Console logging state: on or off")
                        .default_value("on")
                        .value_parser(["on", "off"])
                )
        )
        .subcommand(
            Command::new("status")
                .about("Show this node's status")
        )
}

/// Parses `<ID>` argument text into an [`Id`], reporting a friendly error
/// instead of panicking on malformed input.
fn parse_id(text: &str) -> Option<Id> {
    match Id::try_from(text) {
        Ok(id) => Some(id),
        Err(e) => {
            println!("\x1b[31mInvalid id '{text}': {e}\x1b[0m");
            None
        }
    }
}

fn use_reedline_log_output(external_printer: &ExternalPrinter<String>) {
    let log_sender = external_printer.sender();
    logger::set_console_output_handler(move |line| {
        _ = log_sender.try_send(line);
    });
}

async fn execute_command(
    matches: ArgMatches,
    node: &Node,
    private_key: &PrivateKey,
    readiness: &ConnectionReadiness,
    external_printer: &ExternalPrinter<String>,
) {
    // Reedline only renders its external output while it is reading input.
    // Send logs directly to the terminal until this command has finished.
    logger::set_console_output_handler(|line| println!("{line}"));

    match matches.subcommand() {
        Some(("announcepeer", m)) => {
            let endpoint = m
                .get_one::<String>("ENDPOINT")
                .map(String::as_str)
                .unwrap_or(announce_peer::DEFAULT_ENDPOINT);
            let key = m.get_one::<String>("key").map(String::as_str);
            announce_peer::announce(node, endpoint, key, private_key).await;
        }
        Some(("announcevalue", m)) => {
            let value = m.get_one::<String>("VALUE").unwrap();
            announce_value::announce(node, value).await;
        }
        Some(("findnode", m)) => {
            let Some(target) = parse_id(m.get_one::<String>("ID").unwrap()) else {
                return;
            };
            println!("Attempting to find node with id: {target} ...");
            match node.find_node(&target, None).await {
                Ok(Some(found)) => println!("\x1b[32mFound node: {}\x1b[0m", found),
                Ok(_) => println!("\x1b[32mFound no nodes !!!!\x1b[0m"),
                Err(e) => println!("\x1b[31merror:{}\x1b[0m", e),
            }
        }
        Some(("findpeer", m)) => {
            let Some(peerid) = parse_id(m.get_one::<String>("ID").unwrap()) else {
                return;
            };
            let count = *m.get_one::<usize>("count").unwrap();
            println!("Attempting to find peers with id: {peerid} ...");
            match node.find_peer(&peerid, -1, count, None).await {
                Ok(val) => {
                    if val.is_empty() {
                        println!("\x1b[32mFound no peers !!!\x1b[0m");
                    } else {
                        println!("\x1b[32mFound {} peers, listed below: \x1b[0m", val.len());
                        for (i, item) in val.iter().enumerate() {
                            println!("\x1b[32mpeer [{}]: {}\x1b[0m", i, item);
                        }
                    }
                }
                Err(e) => println!("\x1b[31merror: {}\x1b[0m", e),
            }
        }
        Some(("findvalue", m)) => {
            let Some(valueid) = parse_id(m.get_one::<String>("ID").unwrap()) else {
                return;
            };
            println!("Attempting to find value with id: {valueid} ...");
            match node.find_value(&valueid, -1, None).await {
                Ok(Some(val)) => println!("\x1b[32mFound value: {}\x1b[0m", val),
                Ok(_) => println!("\x1b[32mFound no values !!!!\x1b[0m"),
                Err(e) => println!("\x1b[31merror: {}\x1b[0m", e),
            }
        }
        Some(("keygen", _)) => {
            let keypair = KeyPair::random();
            let id = Id::from(keypair.public_key());
            println!("  User ID     : {}", id.to_base58());
            println!("  DID         : {}", id.to_did_string());
            println!("  Public Key  : {}", keypair.public_key());
            println!("  Private Key : {} (base58)", keypair.private_key().to_base58());
            println!("              : {} (hex)", keypair.private_key().to_hexstr());
            println!("\nKeep the private key secret. It controls this identity.")
        }
        Some(("me", _)) => {
            println!("My key identity:");
            println!("  User ID     : {}", node.id().to_base58());
            println!("  DID         : {}", node.id().to_did_string());
        }
        Some(("log", m )) => match m.get_one::<String>("STATE").map(String::as_str) {
            Some("off") => {
                logger::disable_console_output();
                println!("Console log output disabled. Logs continue in the configured log file.");
            }
            Some("on") => {
                logger::enable_console_output();
                println!("Console log output enabled. Logs continue in the configured log file.");
            }
            _ => unreachable!("clap restricts log state to 'on' or 'off'"),
        },
        Some(("status", _)) => {
            println!("Node id: {}", node.id());
            println!("Node running: {}", node.is_running());
            println!(
                "DHT connection: {}",
                if readiness.is_connected() {
                    "connected"
                } else {
                    "disconnected"
                }
            );
            match node.node_info() {
                Ok(node_info) => println!("Node info: {node_info}"),
                Err(e) => println!("\x1b[31mUnable to read node information: {e}\x1b[0m"),
            }
        }
        _ => {}
    }

    use_reedline_log_output(external_printer);
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opts = Options::parse();

    let mut builder = configuration::Builder::new();
    if let Err(e) = builder.load_from(opts.config.as_deref().unwrap_or("config.yaml")) {
        println!("Loading configuration failed: {e}");
        return;
    }
    if let Some(datadir) = opts.datadir.as_deref() {
        builder.with_data_dir(datadir);
    }
    if let Some(key) = opts.privatekey.as_deref() {
        match PrivateKey::try_from(key) {
            Ok(private_key) => {
                builder.with_private_key(private_key);
            }
            Err(e) => {
                println!("Invalid private key: {e}");
                return;
            }
        }
    }
    if let Some(port) = opts.port {
        builder.with_port(port);
    }

    builder.with_log_console(opts.log);
    let config = match builder.build() {
        Ok(v) => v,
        Err(e) => {
            println!("Loading configuration failed: {e}");
            return;
        }
    };

    #[cfg(feature = "inspect")]
    {
        config.dump();
    }

    let private_key = config.private_key().clone();
    let readiness = Arc::new(ConnectionReadiness::new());

    let node_options = match config.build_node_options() {
        Ok(options) => options,
        Err(e) => {
            println!("Building node options failed: {e}");
            return;
        }
    };

    let node = match Node::new(node_options) {
        Ok(node) => node,
        Err(e) => {
            println!("Creating node failed: {e}");
            return;
        }
    };
    node.add_listener(DefaultConnectionStatusListener {
        readiness: readiness.clone(),
    });
    if let Err(e) = node.start().await {
        println!("Starting node failed: {e}");
        return;
    }

    println!("Waiting for the node to connect to the Boson network...");
    tokio::select! {
        _ = readiness.wait_until_connected() => {}
        result = tokio::signal::ctrl_c() => {
            if let Err(e) = result {
                println!("Waiting for Ctrl-C failed: {e}");
            }
            println!("\nGoodbye!");
            if let Err(e) = node.stop().await {
                println!("Stopping node failed: {e}");
            }
            return;
        }
    }

    let cli = build_cli();
    let external_printer = ExternalPrinter::new(1_024);
    use_reedline_log_output(&external_printer);
    let log_printer = external_printer.clone();
    let mut rl = Reedline::create().with_external_printer(external_printer);
    let prompt = ShellPrompt;

    println!("Welcome to the Boson shell. Type 'help' for a list of commands, 'exit' to quit.\n");

    loop {
        let Ok(sig) = rl.read_line(&prompt) else {
            println!("\nFatal error reading input.");
            continue;
        };
        match sig {
            Signal::Success(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                if input == "exit" || input == "quit" {
                    println!("Goodbye!");
                    break;
                }

                let args: Vec<String> = input.split_whitespace().map(str::to_string).collect();
                match cli.clone().try_get_matches_from(args) {
                    Ok(matches) => {
                        execute_command(
                            matches,
                            &node,
                            &private_key,
                            &readiness,
                            &log_printer,
                        )
                        .await
                    }
                    Err(e) => println!("{e}"),
                }
            }
            Signal::CtrlC | Signal::CtrlD => {
                println!("\nGoodbye!");
                break;
            }
            _ => {}
        }
    }

    if let Err(e) = node.stop().await {
        println!("Stopping node failed: {e}");
    }
}
