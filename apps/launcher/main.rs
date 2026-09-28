use clap::Parser;
use std::process::exit;
use boson::activeproxy::{
    Client as APClient,
    Options as APOptions,
};

const DEFAULT_CONFIG: &str = "apps/launcher/config.yaml";

#[derive(Parser, Debug)]
#[command(name = "launcher")]
#[command(version = "1.0")]
#[command(about = "Boson launcher service", long_about = None)]
struct Options {
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
        .unwrap_or(format!("{DEFAULT_CONFIG}"));

    let opts = match APOptions::load(&config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error building ActiveProxy Options: {e}");
            exit(1);
        }
    };

    let ap = match APClient::new(opts) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error creating ActiveProxy client: {e}");
            exit(1);
        }
    };

    if let Err(e) = ap.start().await {
        eprintln!("Error starting ActiveProxy client: {e}");
        exit(1);
    }

    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Failed to listen for shutdown signal.");
    }

    println!("Shutting down...");
    let _ = ap.stop().await;
}
