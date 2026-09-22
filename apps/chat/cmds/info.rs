use std::sync::Arc;

use clap::{ArgMatches, Command};

use boson::messaging::Client;

pub(crate) fn cli() -> Command {
    Command::new("info")
        .about("Show messaging client identity, endpoints, and connection status")
        .help_template("{subcommands}")
}

pub(crate) async fn execute(_: &ArgMatches, client: &Arc<Client>) {
    println!("User ID:            {}", client.user_id());
    println!("Device ID:          {}", client.device_id());
    println!(
        "Director Node ID:   {}",
        client
            .director_node_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "<unconfigured>".to_string())
    );
    println!(
        "Director Endpoint:  {}",
        client.director_endpoint().unwrap_or("<unconfigured>")
    );
    println!("Messaging Peer ID:  {}", client.service_peer_id());
    println!(
        "Messaging Endpoint: {}",
        client
            .service_endpoint()
            .unwrap_or("<DHT discovery required>")
    );
    println!("Connection Status:  {}", client.connection_status());
}
