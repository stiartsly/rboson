use clap::{ArgMatches, Command, Parser};
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
mod config;
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

#[derive(Parser, Debug)]
#[command(about = "Boson Shell", long_about = None)]
struct Options {
    /// The node configuration file
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value = "apps/shell/node.yaml"
    )]
    config: Option<String>,

    /// Override the node data directory
    #[arg(long, value_name = "DIR")]
    datadir: Option<String>,

    /// Override the node private key
    #[arg(long, value_name = "PRIVATE_KEY")]
    privatekey: Option<String>,

    /// Override the node listen port
    #[arg(long, value_name = "PORT")]
    port: Option<u16>,

    /// Director URL; defaults to BOSON_DIRECTOR_URL or the built-in default
    #[arg(long, value_name = "URL")]
    director_url: Option<String>,

    /// User private key; defaults to BOSON_USER_KEY or the built-in default
    #[arg(long, value_name = "PRIVATE_KEY")]
    userkey: Option<String>,

    /// Accept invalid Director TLS certificates
    #[arg(long)]
    insecure: bool,

    /// Enable log output on the console
    #[arg(long)]
    log: bool,
}

fn build_cli() -> Command {
    Command::new("boson")
        .no_binary_name(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .subcommand(cmds::announce_peer::command())
        .subcommand(cmds::store_value::command())
        .subcommand(cmds::find_node::command())
        .subcommand(cmds::find_peer::command())
        .subcommand(cmds::find_value::command())
        .subcommand(cmds::identity::command())
        .subcommand(cmds::login::command())
        .subcommand(cmds::me::command())
        .subcommand(cmds::device::command())
        .subcommand(cmds::log::command())
        .subcommand(cmds::status::command())
}

fn use_ui_log_output(shell_ui: &ui::ShellUi) {
    let shell_ui = shell_ui.clone();
    logger::set_console_output_handler(move |line| {
        shell_ui.log(line);
    });
}

async fn execute_command(
    matches: ArgMatches,
    node: &Node,
    node_private_key: &PrivateKey,
    shell_config: &config::ShellConfig,
    readiness: &ConnectionReadiness,
    login_session: &mut cmds::login::Session,
) {
    match matches.subcommand() {
        Some(("announcepeer", m)) => {
            cmds::announce_peer::run(m, node, node_private_key).await;
        }
        Some(("announcevalue", m)) => {
            cmds::store_value::run(m, node).await;
        }
        Some(("findnode", m)) => {
            cmds::find_node::run(m, node).await;
        }
        Some(("findpeer", m)) => {
            cmds::find_peer::run(m, node).await;
        }
        Some(("findvalue", m)) => {
            cmds::find_value::run(m, node).await;
        }
        Some(("identity", m)) => {
            cmds::identity::run(m, shell_config.user_private_key());
        }
        Some(("device", m)) => {
            cmds::device::run(m, login_session.client(), node.options().data_dir()).await;
        }
        Some(("login", _)) => cmds::login::run(login_session).await,
        Some(("me", _)) => cmds::me::run(login_session).await,
        Some(("log", m)) => cmds::log::run(m),
        Some(("status", _)) => cmds::status::run(node, readiness.is_connected()),
        _ => {}
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opts = Options::parse();

    let shell_ui = ui::ShellUi::new();
    shell_ui.draw();
    use_ui_log_output(&shell_ui);
    cmds::set_result_output({
        let shell_ui = shell_ui.clone();
        move |line| shell_ui.result(line)
    });

    let shell_config = match config::ShellConfig::new(
        opts.director_url.as_deref(),
        opts.userkey.as_deref(),
        opts.insecure,
    ) {
        Ok(config) => config,
        Err(e) => {
            shell_ui.result(format!("Creating shell configuration failed: {e}"));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            shell_ui.finish();
            return;
        }
    };
    let mut login_session = match cmds::login::Session::new(&shell_config) {
        Ok(session) => session,
        Err(e) => {
            shell_ui.result(format!("Creating Director client failed: {e}"));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            shell_ui.finish();
            return;
        }
    };

    let config = opts
        .config
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("NODE_CONFIG").ok())
        .unwrap_or_else(|| "apps/shell/node.yaml".to_string());

    let mut node_options = match NodeOptions::load(&config) {
        Ok(options) => options,
        Err(e) => {
            shell_ui.result(format!("Loading node configuration failed: {e}"));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            shell_ui.finish();
            return;
        }
    };
    if let Some(datadir) = opts.datadir.as_deref() {
        node_options = node_options.with_data_dir(datadir);
    }
    if let Some(key) = opts.privatekey.as_deref() {
        match PrivateKey::try_from(key) {
            Ok(private_key) => {
                node_options = node_options.with_private_key(private_key);
            }
            Err(e) => {
                shell_ui.result(format!("Invalid private key: {e}"));
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                shell_ui.finish();
                return;
            }
        }
    }
    if let Some(port) = opts.port {
        node_options = node_options.with_port(port);
    }
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
            Ok(None) => {
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

        let args: Vec<String> = input.split_whitespace().map(str::to_string).collect();
        match cli.clone().try_get_matches_from(args) {
            Ok(matches) => {
                execute_command(
                    matches,
                    &node,
                    &node_private_key,
                    &shell_config,
                    &readiness,
                    &mut login_session,
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
            None => cli.write_long_help(&mut output),
        },
        None => cli.write_long_help(&mut output),
    };

    match result {
        Ok(()) => shell_ui.result(String::from_utf8_lossy(&output).into_owned()),
        Err(e) => shell_ui.result(format!("Unable to render help: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BOSON_DIRECTOR_URL, BOSON_USER_KEY};
    use boson::{signature::KeyPair, Id};
    use serial_test::serial;

    #[test]
    fn director_commands_are_available() {
        assert_eq!(
            build_cli()
                .try_get_matches_from(["login"])
                .unwrap()
                .subcommand_name(),
            Some("login")
        );
        assert_eq!(
            build_cli()
                .try_get_matches_from(["me"])
                .unwrap()
                .subcommand_name(),
            Some("me")
        );
        assert_eq!(
            build_cli()
                .try_get_matches_from(["device", "--list"])
                .unwrap()
                .subcommand_name(),
            Some("device")
        );
    }

    #[test]
    fn identity_command_replaces_keygen() {
        assert_eq!(
            build_cli()
                .try_get_matches_from(["identity"])
                .unwrap()
                .subcommand_name(),
            Some("identity")
        );
        assert!(build_cli().try_get_matches_from(["identity", "-g"]).is_ok());
        assert!(build_cli().try_get_matches_from(["keygen"]).is_err());
    }

    #[test]
    fn director_url_and_userkey_are_optional_arguments() {
        let defaults = Options::try_parse_from(["shell"]).unwrap();
        assert_eq!(defaults.director_url, None);
        assert_eq!(defaults.userkey, None);

        let overridden = Options::try_parse_from([
            "shell",
            "--director-url",
            "https://director.example",
            "--userkey",
            "0x00",
        ])
        .unwrap();
        assert_eq!(
            overridden.director_url.as_deref(),
            Some("https://director.example")
        );
        assert_eq!(overridden.userkey.as_deref(), Some("0x00"));
    }

    #[test]
    #[serial]
    fn shell_config_reads_director_url_from_environment() {
        unsafe {
            env::set_var(BOSON_DIRECTOR_URL, "https://env-director.example");
            env::remove_var(BOSON_USER_KEY);
        }

        let shell_config = config::ShellConfig::new(None, None, false).unwrap();
        assert_eq!(
            shell_config
                .director_options()
                .unwrap()
                .director_url()
                .as_str(),
            "https://env-director.example/"
        );

        unsafe {
            env::remove_var(BOSON_DIRECTOR_URL);
        }
    }

    #[test]
    #[serial]
    fn shell_config_reads_userkey_from_environment() {
        let user_key = KeyPair::random();
        unsafe {
            env::remove_var(BOSON_DIRECTOR_URL);
            env::set_var(BOSON_USER_KEY, user_key.private_key().to_string());
        }

        let shell_config = config::ShellConfig::new(None, None, false).unwrap();
        assert_eq!(shell_config.user_id(), Id::from(user_key.public_key()));

        unsafe {
            env::remove_var(BOSON_USER_KEY);
        }
    }
}
