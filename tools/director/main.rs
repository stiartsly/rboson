use boson::director::{self, Client, NodeStatus, Service};
use boson::{signature, Id, Result};
use clap::Parser;
use std::{env, time::UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(name = "director")]
#[command(
    about = "List public information and advertised services from a Boson Director node",
    long_about = None
)]
struct Options {
    /// Director node URL
    #[arg(long, value_name = "URL")]
    director_url: Option<String>,

    /// User private key used when authenticated Director calls are needed
    #[arg(short = 'u', long = "user-key", value_name = "PRIVATE_KEY")]
    user_key: Option<String>,

    /// Accept invalid TLS certificates from the Director endpoint
    #[arg(long)]
    insecure: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("director: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let options = Options::parse();
    let director_url = options
        .director_url
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("BOSON_DIRECTOR_URL").ok())
        .ok_or_else(|| {
            boson::errors::ArgumentError::new(
                "director URL must be provided with --director-url or BOSON_DIRECTOR_URL",
            )
        })?;
    let userkey = options
        .user_key
        .as_deref()
        .map(str::to_owned)
        .or_else(|| env::var("USER_PRIVATE_KEY").ok())
        .map(|key| signature::PrivateKey::try_from(key.as_str()))
        .transpose()?;

    let mut options = director::Options::new(&director_url)?.with_insecure(options.insecure);
    if let Some(userkey) = userkey {
        options = options.with_user_private_key(userkey);
    }

    let client = Client::new(options)?;
    let status = client.fetch_node_status().await?;

    print_status(&status, client.user_id());
    Ok(())
}

fn print_status(status: &NodeStatus, authenticated_user: Option<&Id>) {
    println!("+------------------------------------------------------------+");
    println!("|                  Boson Super Node Information              |");
    println!("+------------------------------------------------------------+");
    print_field("Node ID", status.node_id());
    print_field("Software", status.software().unwrap_or("-"));
    print_field("Version", status.version().unwrap_or("-"));
    print_field("Name", status.name().unwrap_or("-"));
    print_field("Website", status.website().unwrap_or("-"));
    print_field("Contact", status.contact().unwrap_or("-"));
    print_field("Logo", status.logo().unwrap_or("-"));
    print_field("Running", if status.is_running() { "yes" } else { "no" });
    print_field("Started At", format_started_at(status));
    print_field(
        "Authenticated User",
        authenticated_user
            .map(ToString::to_string)
            .unwrap_or_else(|| "-".to_string()),
    );

    println!();
    print_services(status.services());
}

fn print_field(label: &str, value: impl std::fmt::Display) {
    println!("{label:>20}: {value}");
}

fn format_started_at(status: &NodeStatus) -> String {
    match status.started_at().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{} seconds since Unix epoch", duration.as_secs()),
        Err(_) => "before Unix epoch".to_string(),
    }
}

fn print_services(services: &[Service]) {
    println!("+------------------------------------------------------------+");
    println!("|                    Advertised Services                     |");
    println!("+------------------------------------------------------------+");

    if services.is_empty() {
        println!("No services advertised by this Director node.");
        return;
    }

    let peer_id_width = services
        .iter()
        .map(|service| service.peer_id().to_string().len())
        .max()
        .unwrap_or("Peer ID".len())
        .max("Peer ID".len());

    println!(
        "{:<4} {:<16} {:<20} {:<peer_id_width$} {}",
        "#", "Service", "Name", "Peer ID", "Endpoint"
    );
    println!("{}", "-".repeat(66 + peer_id_width));
    for (index, service) in services.iter().enumerate() {
        let peer_id = service.peer_id().to_string();
        println!(
            "{:<4} {:<16} {:<20} {:<peer_id_width$} {}",
            index + 1,
            service.service_id(),
            service.service_name().unwrap_or("-"),
            peer_id,
            service.endpoint().unwrap_or("-")
        );
    }
}
