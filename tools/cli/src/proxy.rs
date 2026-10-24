// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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
