use clap::Args;
use std::process::exit;
use boson::activeproxy::{
    Client as APClient,
    Options as APOptions,
};

const DEFAULT_CONFIG: &str = "tools/cli/proxy.yaml";

#[derive(Args, Debug)]
pub(crate) struct Options {
    /// The active proxy configuration file
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    config: Option<String>,
}

pub(crate) async fn start(options: Options) {
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
