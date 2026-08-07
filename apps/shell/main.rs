use std::{sync::Arc, time::Duration};
use clap::{arg, Command, ArgMatches, Parser};
use reedline::{Reedline, Signal, Prompt, PromptEditMode, PromptHistorySearch};
use std::borrow::Cow;

use boson::{
    Id,
    Network,
    NodeConfig,
    cfg::configuration,
    signature::PrivateKey,
    dht::{
        Node,
        ConnectionStatus,
        ConnectionStatusListener,
    },
};

mod announce_peer;
mod announce_value;

/// Logs connection status changes and, once connected, signals readiness to
/// any task awaiting it (used while waiting to enter the interactive shell).
#[derive(Default)]
struct DefaultConnectionStatusListener {
    ready: Option<Arc<tokio::sync::Notify>>,
}

impl ConnectionStatusListener for DefaultConnectionStatusListener {
    fn status_changed(&self,
        network: Network,
        new_status: ConnectionStatus,
        old_status: ConnectionStatus,
    ) {
        println!("\x1b[32mConnection status changed for network {}: {}->{}\x1b[0m", network, old_status, new_status);
    }
    fn connecting(&self, network: Network) {
        println!("\x1b[32mConnecting to network {}...\x1b[0m", network);
    }
    fn connected(&self, network: Network) {
        println!("\x1b[32mConnected to network {}.\x1b[0m", network);
        if let Some(ready) = self.ready.as_ref() {
            ready.notify_one();
        }
    }
    fn disconnected(&self, network: Network) {
        println!("\x1b[32mDisconnected from network {}.\x1b[0m", network);
    }
}

struct ShellPrompt;
impl Prompt for ShellPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> { "boson> ".into() }
    fn render_prompt_right(&self) -> Cow<'_, str> { "".into() }
    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> { "".into() }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> { "... ".into() }
    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<'_, str> { "".into() }
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
}

/// Builds the interactive shell's subcommand tree.
///
/// Sub commands:
///   - `announce_peer [ENDPOINT] [-k/--key <PRIVATE_KEY>]`
///   - `announce_value <VALUE>`
///   - `find_node <ID>`
///   - `find_peer <ID> [-c/--count <COUNT>]`
///   - `find_value <ID>`
///   - `status`
fn build_cli() -> Command {
    Command::new("boson")
        .no_binary_name(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .subcommand(
            Command::new("announce_peer")
                .about("Announce a peer to the Boson network")
                .arg(arg!([ENDPOINT] "Endpoint value for the announced peer")
                    .default_value(announce_peer::DEFAULT_ENDPOINT))
                .arg(arg!(-k --key <PRIVATE_KEY> "Private key (hex or base58) for the peer identity; \
                    defaults to this node's own key")
                    .required(false))
        )
        .subcommand(
            Command::new("announce_value")
                .about("Announce an immutable value to the Boson network")
                .arg(arg!(<VALUE> "Value data (string) to announce"))
        )
        .subcommand(
            Command::new("find_node")
                .about("Look up a node by id")
                .arg(arg!(<ID> "Target node id (base58)"))
        )
        .subcommand(
            Command::new("find_peer")
                .about("Look up peers announced under an id")
                .arg(arg!(<ID> "Target peer id (base58)"))
                .arg(arg!(-c --count <COUNT> "Expected number of peers")
                    .default_value("8"))
        )
        .subcommand(
            Command::new("find_value")
                .about("Look up a value by id")
                .arg(arg!(<ID> "Target value id (base58)"))
        )
        .subcommand(
            Command::new("status")
                .about("Show this node's id")
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

async fn execute_command(matches: ArgMatches, node: &Node, private_key: &PrivateKey) {
    match matches.subcommand() {
        Some(("announce_peer", m)) => {
            let endpoint = m.get_one::<String>("ENDPOINT")
                .map(String::as_str)
                .unwrap_or(announce_peer::DEFAULT_ENDPOINT);
            let key = m.get_one::<String>("key").map(String::as_str);
            announce_peer::announce(node, endpoint, key, private_key).await;
        }
        Some(("announce_value", m)) => {
            let value = m.get_one::<String>("VALUE").unwrap();
            announce_value::announce(node, value).await;
        }
        Some(("find_node", m)) => {
            let Some(target) = parse_id(m.get_one::<String>("ID").unwrap()) else { return };
            println!("Attemp finding node with id: {} ...", target);
            match node.find_node(&target, None).await {
                Ok(Some(found)) => println!("\x1b[32mFound node: {}\x1b[0m", found),
                Ok(_) => println!("\x1b[32mFound no nodes !!!!\x1b[0m"),
                Err(e) => println!("\x1b[31merror:{}\x1b[0m", e),
            }
        }
        Some(("find_peer", m)) => {
            let Some(peerid) = parse_id(m.get_one::<String>("ID").unwrap()) else { return };
            let count: usize = m.get_one::<String>("count")
                .and_then(|v| v.parse().ok())
                .unwrap_or(8);
            println!("Attemp finding peers with id: {} ...", peerid);
            match node.find_peer(&peerid, -1, count, None).await {
                Ok(val) => {
                    if val.is_empty() {
                        println!("\x1b[32mFound no peers !!!\x1b[0m");
                    } else {
                        println!("Found {} peers, listed below: ", val.len());
                        for (i, item) in val.iter().enumerate() {
                            println!("peer [{}]: {}", i, item);
                        }
                    }
                },
                Err(e) => println!("error: {}", e),
            }
        }
        Some(("find_value", m)) => {
            let Some(valueid) = parse_id(m.get_one::<String>("ID").unwrap()) else { return };
            println!("Attemp finding value with id: {} ...", valueid);
            match node.find_value(&valueid, -1, None).await {
                Ok(Some(val)) => println!("Found value: {}", val),
                Ok(None) => println!("\x1b[32mFound no values !!!!\x1b[0m"),
                Err(e) => println!("error: {}", e),
            }
        }
        Some(("status", _)) => {
            println!("Node id: {}", node.id());
            //println!("Node status: {}", node.status());
        }
        _ => {}
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opts = Options::parse();

    let mut builder = configuration::Builder::new();
    if let Err(e) = builder.load(opts.config.as_deref().unwrap_or("config.yaml")) {
        println!("Loading configuration failed: {e}");
        return;
    }
    if let Some(datadir) = opts.datadir.as_deref() {
        builder.with_data_dir(datadir);
    }
    if let Some(key) = opts.privatekey.as_deref() {
        match PrivateKey::try_from(key) {
            Ok(private_key) => { builder.with_private_key(private_key); }
            Err(e) => {
                println!("Invalid private key: {e}");
                return;
            }
        }
    }
    if let Some(port) = opts.port {
        builder.with_port(port);
    }
    let config = match builder.build() {
        Ok(v) => v,
        Err(e) => {
            println!("Loading configuration failed: {e}");
            return;
        }
    };

    #[cfg(feature = "inspect")] {
        config.dump();
    }

    let private_key = config.private_key().clone();
    let ready = Arc::new(tokio::sync::Notify::new());

    let node = Node::new(Box::new(config)).unwrap();
    node.add_listener(DefaultConnectionStatusListener { ready: Some(ready.clone()) });
    let _ = node.start().await;

    println!("Waiting for the node to connect to the Boson network...");
    if tokio::time::timeout(Duration::from_secs(30), ready.notified()).await.is_err() {
        println!("\x1b[33mTimed out waiting for a network connection; the shell is still usable.\x1b[0m");
    }

    let cli = build_cli();
    let mut rl = Reedline::create();
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
                    Ok(matches) => execute_command(matches, &node, &private_key).await,
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

    let _ = node.stop().await;
}
