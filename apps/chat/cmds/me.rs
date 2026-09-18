use std::sync::Arc;

use clap::{ArgMatches, Command};

use boson::messaging::MessagingClient;

pub(crate) fn cli() -> Command {
    Command::new("me")
        .about("Show client information")
        .help_template("{subcommands}")
}

pub(crate) async fn execute(_: &ArgMatches, client: &Arc<dyn MessagingClient>) {
    println!("user id:    {}", client.user_id());
    println!("device id:  {}", client.device_id());
    println!("service id: {}", client.service_peer_id());
    println!(
        "endpoint:   {}",
        client
            .service_endpoint()
            .unwrap_or("<DHT discovery required>")
    );
    println!(
        "running={} connected={} ready={}",
        client.is_running(),
        client.is_connected(),
        client.is_ready()
    );
}
