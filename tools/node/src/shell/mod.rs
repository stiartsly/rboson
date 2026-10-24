use clap::{ArgMatches, Args, Command};
use std::{
    env,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use boson::{
    core::logger,
    dht::{ConnectionStatus, ConnectionStatusListener, Node, NodeOptions},
    signature::PrivateKey,
    Network,
};
use log::{debug, info, warn};

mod cmds;
mod ui;

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

#[derive(Args, Debug)]
pub(crate) struct Options {
    /// The node configuration file
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value = "tools/node/shell/node.yaml"
    )]
    config: Option<String>,

    /// Enable log output on the console
    #[arg(long)]
    log: bool,
}

fn build_cli() -> Command {
    Command::new("boson")
        .no_binary_name(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .subcommand(cmds::info::command())
        .subcommand(pending("bootstrap", "Join the DHT through a node already on it."))
        .subcommand(
            group("find", "Look up a node, a value or a peer.")
                .subcommand(cmds::find_node::command())
                .subcommand(cmds::find_value::command())
                .subcommand(cmds::find_peer::command()),
        )
        .subcommand(group("store", "Store a value on the DHT.").subcommand(cmds::store_value::command()))
        .subcommand(
            group("announce", "Announce a service peer on the DHT.")
                .subcommand(cmds::announce_peer::command()),
        )
        .subcommand(
            group("subscribe", "Watch a value or peer records for changes.")
                .subcommand(pending("value", "Watch a value for newer versions."))
                .subcommand(pending("peer", "Watch the peer records under an id."))
                .subcommand(pending("list", "List what is watched.")),
        )
        .subcommand(pending("unsubscribe", "Stop watching a value or peer records."))
        .subcommand(cmds::rt::command())
        .subcommand(
            group("storage", "Show the values and peers stored on this node.")
                .subcommand(pending("values", "List the values stored on this node."))
                .subcommand(pending("value", "Show a value stored on this node."))
                .subcommand(pending("peers", "List the peers stored on this node."))
                .subcommand(pending("peer", "Show the peers stored for one service.")),
        )
        .subcommand(pending("cache", "Show a routing table saved in a data directory."))
        .subcommand(pending("keygen", "Generate a key pair."))
        .subcommand(pending("stop", "Stop the node. The shell stays open; 'exit' leaves it."))
        .subcommand(cmds::log::command())
}

/// A command group of the shell, such as `find`.
fn group(name: &'static str, about: &'static str) -> Command {
    Command::new(name).about(about).subcommand_required(true)
}

/// A command of the Java boson-node shell that has no Rust implementation yet.
fn pending(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(clap::Arg::new("args").num_args(0..).trailing_var_arg(true).allow_hyphen_values(true).hide(true))
}

/// The command names of earlier versions of this shell, as their current form.
fn renamed(args: Vec<String>) -> Vec<String> {
    let replacement: &[&str] = match args.first().map(String::as_str) {
        Some("findnode") => &["find", "node"],
        Some("findvalue") => &["find", "value"],
        Some("findpeer") => &["find", "peer"],
        Some("announcepeer") => &["announce", "peer"],
        Some("announcevalue") => &["store", "value"],
        _ => return args,
    };
    replacement.iter().map(|s| s.to_string()).chain(args.into_iter().skip(1)).collect()
}

/// The subcommand names of a parsed command line, such as `storage values`.
fn command_path(matches: &ArgMatches) -> String {
    let mut names = Vec::new();
    let mut current = matches;
    while let Some((name, sub)) = current.subcommand() {
        names.push(name);
        current = sub;
    }
    names.join(" ")
}

fn use_ui_log_output(shell_ui: &ui::ShellUi) {
    let shell_ui = shell_ui.clone();
    logger::set_console_output_handler(move |line| {
        shell_ui.log(ui::colorize_log_line(&line));
    });
}

async fn execute_command(
    matches: ArgMatches,
    node: &Node,
    node_private_key: &PrivateKey,
    readiness: &ConnectionReadiness,
) {
    match matches.subcommand() {
        Some(("announce", m)) => match m.subcommand() {
            Some(("peer", m)) => cmds::announce_peer::run(m, node, node_private_key).await,
            _ => {}
        },
        Some(("store", m)) => match m.subcommand() {
            Some(("value", m)) => cmds::store_value::run(m, node).await,
            _ => {}
        },
        Some(("find", m)) => match m.subcommand() {
            Some(("node", m)) => cmds::find_node::run(m, node).await,
            Some(("peer", m)) => cmds::find_peer::run(m, node).await,
            Some(("value", m)) => cmds::find_value::run(m, node).await,
            _ => {}
        },
        Some(("id", _)) => {
            cmds::info::run(node, readiness.is_connected());
        }
        Some(("log", m)) => {
            cmds::log::run(m);
        }
        Some(("routing", m)) => {
            cmds::rt::run(m, node);
        }
        Some(_) => {
            cmds::print_result(format!("{}: not implemented yet", command_path(&matches)));
        }
        None => {}
    }
}

pub(crate) async fn run(opts: Options) {

    let shell_ui = ui::ShellUi::new();
    shell_ui.draw();
    use_ui_log_output(&shell_ui);
    cmds::set_result_output({
        let shell_ui = shell_ui.clone();
        move |line| shell_ui.result(line)
    });

    let config = opts
        .config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("NODE_CONFIG").ok())
        .unwrap_or_else(|| "tools/node/shell/node.yaml".to_string());

    let node_options = match NodeOptions::load(&config) {
        Ok(options) => options,
        Err(e) => {
            shell_ui.result(format!("Loading node configuration failed: {e}"));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            shell_ui.finish();
            return;
        }
    };
    let log_console = opts.log || node_options.log_console_enabled();

    let node_private_key = node_options.private_key().clone();
    let readiness = Arc::new(ConnectionReadiness::new());

    // Temporarily disable log console during Node::new so raw output does not escape before redirection
    let node_options_for_init = node_options.clone().with_log_console(false);
    let node = match Node::new(node_options_for_init) {
        Ok(node) => node,
        Err(e) => {
            shell_ui.result(format!("Creating node failed: {e}"));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            shell_ui.finish();
            return;
        }
    };

    // Re-install console output handler to route all logs to Log pane
    use_ui_log_output(&shell_ui);
    if log_console {
        logger::enable_console_output();
    }

    node.add_listener(DefaultConnectionStatusListener {
        readiness: readiness.clone(),
    });
    if let Err(e) = node.start().await {
        shell_ui.result(format!("Starting node failed: {e}"));
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        shell_ui.finish();
        return;
    }

    shell_ui.result("Waiting for the node to connect to the Boson network...");
    tokio::select! {
        _ = readiness.wait_until_connected() => {}
        result = tokio::signal::ctrl_c() => {
            if let Err(e) = result {
                shell_ui.result(format!("Waiting for Ctrl-C failed: {e}"));
            }
            shell_ui.result("Goodbye!");
            if let Err(e) = node.stop().await {
                shell_ui.result(format!("Stopping node failed: {e}"));
            }
            shell_ui.finish();
            return;
        }
    }

    let mut cli = build_cli();
    shell_ui
        .result("Welcome to the Boson shell. Type 'help' for a list of commands, 'exit' to quit.");

    loop {
        let line = match shell_ui.read_line().await {
            Ok(Some(line)) => line,
            Ok(_) => {
                shell_ui.result("Goodbye!");
                break;
            }
            Err(_) => continue,
        };
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        shell_ui.result(format!("Command input: {input}"));
        if input == "exit" || input == "quit" {
            shell_ui.result("Goodbye!");
            break;
        }
        if input == "help" {
            print_help(&mut cli, None, &shell_ui);
            continue;
        }
        if let Some(name) = input.strip_prefix("help ") {
            print_help(&mut cli, Some(name.trim()), &shell_ui);
            continue;
        }

        let args = renamed(input.split_whitespace().map(str::to_string).collect());
        match cli.clone().try_get_matches_from(args) {
            Ok(matches) => {
                execute_command(
                    matches,
                    &node,
                    &node_private_key,
                    &readiness,
                )
                .await
            }
            Err(e) => shell_ui.result(e.to_string()),
        }
    }

    shell_ui.finish();
    if let Err(e) = node.stop().await {
        eprintln!("Stopping node failed: {e}");
    }
}

fn print_help(cli: &mut Command, command_name: Option<&str>, shell_ui: &ui::ShellUi) {
    let mut output = Vec::new();
    let result = match command_name {
        Some(name) => match cli.find_subcommand_mut(name) {
            Some(command) => command.write_long_help(&mut output),
            _ => cli.write_long_help(&mut output),
        },
        _ => cli.write_long_help(&mut output),
    };

    match result {
        Ok(()) => shell_ui.result(String::from_utf8_lossy(&output).into_owned()),
        Err(e) => shell_ui.result(format!("Unable to render help: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> String {
        let args = renamed(line.split_whitespace().map(str::to_string).collect());
        command_path(&build_cli().try_get_matches_from(args).unwrap())
    }

    #[test]
    fn command_tree_is_valid() {
        build_cli().debug_assert();
    }

    #[test]
    fn earlier_command_names_still_work() {
        assert_eq!(parse("findnode FyHfVWtscJWUeejGQaJXyUnjUcKFGSVVYmozqBuJSmjo"), "find node");
        assert_eq!(parse("announcevalue hello"), "store value");
        assert_eq!(parse("rt"), "routing");
        assert_eq!(parse("info"), "id");
        assert_eq!(parse("storage values"), "storage values");
    }
}
