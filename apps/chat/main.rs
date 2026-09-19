use clap::Parser;
use reedline::{ExternalPrinter, Reedline, Signal};
use std::env;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use boson::{
    core::logger,
    dht::{ConnectionStatus, ConnectionStatusListener, Node, NodeOptions},
    errors::Result,
    messaging::{
        Channel, ChannelListener, Client, ConnectionListener, Contact, ContactListener,
        FriendRequestListener, Message, MessageListener, MessagingClient,
        Options as MessagingOptions, SessionInfo, SessionListener,
    },
    Id, Network,
};
use log::{debug, info, warn};

mod cmds;
mod prompt;
use prompt::MyPrompt;

const BOSON_PEER_ID: &str = "BOSON_MESSAGING_PEER_ID";
const BOSON_ENDPOINT: &str = "BOSON_MESSAGING_PEER_ENDPOINT";
const BOSON_USER_ID: &str = "BOSON_USER_ID";
const BOSON_USER_KEY: &str = "BOSON_USER_KEY";
const BOSON_DEV_KEY: &str = "BOSON_DEV_KEY";
const BOSON_DEVICE_KEY: &str = "BOSON_DEVICE_KEY";
const DEFAULT_NODE_CONFIG: &str = "apps/chat/node.yaml";

#[derive(Parser, Debug)]
#[command(name = "chat", version = "1.0", about = "Boson messaging chat")]
struct Options {
    /// Node configuration file used to start the chat Boson node.
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// Messaging configuration file (e.g. apps/chat/bob.yaml).
    #[arg(short = 'm', long = "messaging-config", value_name = "FILE")]
    messaging_config: Option<String>,

    /// Messaging service peer id.
    #[arg(long, value_name = "PEERID")]
    peerid: Option<String>,

    /// Messaging service endpoint, for example mqtt://127.0.0.1:1883.
    #[arg(long, value_name = "ENDPOINT")]
    endpoint: Option<String>,

    /// User private key alias (--userkey).
    #[arg(long = "userkey", value_name = "PRIVATE_KEY")]
    userkey: Option<String>,

    /// Device private key. BOSON_DEV_KEY or BOSON_DEVICE_KEY is used as fallback.
    #[arg(long = "dev-key", value_name = "PRIVATE_KEY")]
    dev_key: Option<String>,

    /// Override the DHT node UDP port from the node configuration.
    #[arg(long, value_name = "PORT")]
    port: Option<u16>,

    /// Override the DHT node data directory from the node configuration.
    #[arg(long, value_name = "DIR")]
    datadir: Option<String>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("chat: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let options = Options::parse();
    let node_options = load_node_options(&options)?;
    let chat_options = load_chat_options(&options)?;
    let external_printer = ExternalPrinter::new(1_024);
    let output = ConsoleOutput::new(external_printer.clone());

    let node = Node::new(node_options)?;
    use_reedline_log_output(&external_printer);
    let readiness = Arc::new(ConnectionReadiness::new());
    node.add_listener(DefaultConnectionStatusListener {
        readiness: readiness.clone(),
    });
    node.start().await?;
    println!("Boson DHT node {} is up and running.", node.id());

    let client: Arc<Client> = Arc::new(Client::new(chat_options));
    client.add_connection_listener(Arc::new(ConsoleConnectionListener::new(output.clone())));
    client.add_message_listener(Arc::new(ConsoleMessageListener::new(output.clone())));
    client.add_channel_listener(Arc::new(ConsoleChannelListener::new(output.clone())));
    client.add_contact_listener(Arc::new(ConsoleContactListener::new(output.clone())));
    client.add_session_listener(Arc::new(ConsoleSessionListener::new(output.clone())));
    client.add_friend_request_listener(Arc::new(ConsoleFriendRequestListener::new(output)));

    client.start().await?;

    let mut cli = cmds::build_cli();
    let mut editor = Reedline::create().with_external_printer(external_printer);
    let prompt = MyPrompt;
    println!("Welcome to the messaging shell. Type 'help' or 'exit'.");

    loop {
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let args: Vec<String> = line.split_whitespace().map(ToString::to_string).collect();
                if args.is_empty() {
                    continue;
                }
                if matches!(args[0].as_str(), "exit" | "quit") {
                    break;
                }
                if args[0] == "help" {
                    if let Some(name) = args.get(1) {
                        match cli.find_subcommand_mut(name) {
                            Some(command) => command.print_long_help()?,
                            None => cli.print_long_help()?,
                        }
                    } else {
                        cli.print_long_help()?;
                    }
                    println!();
                    continue;
                }
                match cli.clone().try_get_matches_from(args) {
                    Ok(matches) => cmds::execute_command(matches, &client).await,
                    Err(error) => println!("{error}"),
                }
            }
            Ok(Signal::CtrlC | Signal::CtrlD) => break,
            Ok(_) => continue,
            Err(error) => {
                println!("Input error: {error}");
                break;
            }
        }
    }

    client.stop().await?;
    node.stop().await?;
    Ok(())
}

fn use_reedline_log_output(external_printer: &ExternalPrinter<String>) {
    let log_sender = external_printer.sender();
    logger::set_console_output_handler(move |line| {
        _ = log_sender.try_send(line);
    });
}

fn load_node_options(options: &Options) -> Result<NodeOptions> {
    let path = options
        .config
        .as_deref()
        .or(options.config.as_deref())
        .unwrap_or(DEFAULT_NODE_CONFIG);

    let mut opts = NodeOptions::load(path)?;
    if let Some(port) = options.port {
        opts = opts.with_port(port);
    }
    if let Some(datadir) = options.datadir.as_deref() {
        opts = opts.with_data_dir(datadir);
    }
    Ok(opts)
}

fn load_chat_options(options: &Options) -> Result<MessagingOptions> {
    let mut opts = match options.messaging_config.as_deref() {
        Some(path) => MessagingOptions::load(path)?,
        _ => MessagingOptions::new(),
    };

    if let Some(peerid_str) = options
        .peerid
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var(BOSON_PEER_ID).ok())
    {
        opts = opts.with_service_peerid(Id::try_from(peerid_str.as_str())?);
    }

    if let Some(endpoint) = options
        .endpoint
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var(BOSON_ENDPOINT).ok())
    {
        opts = opts.with_service_endpoint(endpoint)?;
    }

    if let Some(user_key) = options
        .userkey
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var(BOSON_USER_KEY).ok())
        .or_else(|| env::var(BOSON_USER_ID).ok())
    {
        opts = opts.with_user_key_str(&user_key)?;
    }

    if let Some(device_key) = options
        .dev_key
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var(BOSON_DEV_KEY).ok())
        .or_else(|| env::var(BOSON_DEVICE_KEY).ok())
    {
        opts = opts.with_device_key_str(&device_key)?;
    }

    if opts.user_key().is_none() && opts.user_id().is_none() {
        opts = opts.with_generated_user_key();
    }
    if opts.device_key().is_none() {
        opts = opts.with_generated_device_key();
    }

    Ok(opts)
}

#[derive(Clone)]
struct ConsoleOutput {
    printer: ExternalPrinter<String>,
}

impl ConsoleOutput {
    fn new(printer: ExternalPrinter<String>) -> Self {
        Self { printer }
    }

    fn println(&self, line: impl Into<String>) {
        _ = self.printer.sender().try_send(line.into());
    }
}

struct ConsoleConnectionListener {
    output: ConsoleOutput,
}

impl ConsoleConnectionListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl ConnectionListener for ConsoleConnectionListener {
    fn on_connecting(&self) {
        self.output.println("Connecting to messaging service...");
    }

    fn on_connected(&self) {
        self.output.println("Connected to messaging service");
    }

    fn on_ready(&self) {
        self.output.println("Messaging service is ready");
    }

    fn on_disconnected(&self) {
        self.output.println("Disconnected from messaging service");
    }
}

struct ConsoleMessageListener {
    output: ConsoleOutput,
}

impl ConsoleMessageListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl MessageListener for ConsoleMessageListener {
    fn on_message(&self, message: &dyn Message) {
        self.output
            .println(format!("Received message {}", message.id()));
    }

    fn on_sent(&self, message: &dyn Message) {
        self.output
            .println(format!("Sent message {}", message.id()));
    }
}

struct ConsoleContactListener {
    output: ConsoleOutput,
}

impl ConsoleContactListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl ContactListener for ConsoleContactListener {
    fn on_contact_added(&self, contact: &dyn Contact) {
        self.output
            .println(format!("Contact added: {}", contact.id()));
    }

    fn on_contacts_updated(&self, contacts: &[Box<dyn Contact>]) {
        self.output
            .println(format!("Updated {} contact(s)", contacts.len()));
    }

    fn on_contacts_removed(&self, contact_ids: &[Id]) {
        self.output
            .println(format!("Removed {} contact(s)", contact_ids.len()));
    }

    fn on_contacts_cleared(&self) {
        self.output.println("Contacts cleared");
    }
}

struct ConsoleChannelListener {
    output: ConsoleOutput,
}

impl ConsoleChannelListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl ChannelListener for ConsoleChannelListener {
    fn on_channel_created(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Channel created: {}", channel.id()));
    }

    fn on_channel_deleted(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Channel deleted: {}", channel.id()));
    }

    fn on_joined_channel(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Joined channel: {}", channel.id()));
    }

    fn on_left_channel(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Left channel: {}", channel.id()));
    }
}

struct ConsoleSessionListener {
    output: ConsoleOutput,
}

impl ConsoleSessionListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl SessionListener for ConsoleSessionListener {
    fn on_new_session(&self, session: &SessionInfo) {
        self.output
            .println(format!("New device session: {}", session.device_id()));
    }
}

struct ConsoleFriendRequestListener {
    output: ConsoleOutput,
}

impl ConsoleFriendRequestListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl FriendRequestListener for ConsoleFriendRequestListener {
    fn on_friend_request(&self, user_id: &Id, hello: Option<&str>) {
        self.output.println(format!(
            "Friend request from {}: {}",
            user_id,
            hello.unwrap_or("<no greeting>")
        ));
    }

    fn on_friend_request_accepted(&self, user_id: &Id) {
        self.output
            .println(format!("Friend request accepted by {user_id}"));
    }
}

struct ConnectionReadiness {
    connected_networks: AtomicU8,
}

impl ConnectionReadiness {
    fn new() -> Self {
        Self {
            connected_networks: AtomicU8::new(0),
        }
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
        debug!("DHT {network} status changed: {old_status}->{new_status}");
    }

    fn connecting(&self, network: Network) {
        info!("Connecting to DHT network {network}...");
    }

    fn connected(&self, network: Network) {
        info!("Connected to DHT network {network}");
        self.readiness.set_network_connected(network, true);
    }

    fn disconnected(&self, network: Network) {
        warn!("Disconnected from DHT network {network}");
        self.readiness.set_network_connected(network, false);
    }
}
