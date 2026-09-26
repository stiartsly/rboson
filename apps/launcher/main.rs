use clap::Parser;
use std::process::exit;

use boson::activeproxy::{
    Client as ActiveProxyClient,
    Options as ActiveProxyOptions
};

const DEFAULT_ACTIVEPROXY_CONFIG: &str = "apps/launcher/activeproxy.yaml";

#[derive(Parser, Debug)]
#[command(name = "launcher")]
#[command(version = "1.0")]
#[command(about = "Boson launcher service", long_about = None)]
struct Options {
    /// The ActiveProxy configuration file
    #[arg(long, value_name = "FILE")]
    config: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let options = Options::parse();

    let config = options
        .config
        .as_deref()
        .map(str::to_owned)
        .unwrap_or(format!("{DEFAULT_ACTIVEPROXY_CONFIG}"));

    let ap_opts = match ActiveProxyOptions::load(&config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error building ActiveProxy Options: {e}");
            exit(1);
        }
    };

    let ap = match ActiveProxyClient::new(None, ap_opts) {
        Ok(ap) => ap,
        Err(e) => {
            eprintln!("Creating ActiveProxy client error: {e}");
            exit(1);
        }
    };

    if let Err(e) = ap.start().await {
        eprintln!("ActiveProxy client stopped with error: {e}");
    }

    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Failed to listen for shutdown signal.");
    }

    println!("Shutting down...");
    let _ = ap.stop().await;
}
