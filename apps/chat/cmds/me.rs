use std::sync::Arc;
use clap::{ArgMatches, Command};
use boson::messaging::Client;

pub(crate) fn cli() -> Command {
    Command::new("me")
        .about("Show user and client information")
        .help_template("{subcommands}")
}

pub(crate) async fn execute(_: &ArgMatches, client: &Arc<Client>) {
    println!("User Id:            {}", client.user_id());
    println!("Device Id:          {}", client.device_id());
    println!("Messaging Peer Id:  {}", client.service_peer_id());
    println!(
        "Messaging endpoint: {}",
        client
            .service_endpoint()
            .unwrap_or("<DHT discovery required>")
    );
    println!("Connection Status:  {}", client.connection_status());
}
