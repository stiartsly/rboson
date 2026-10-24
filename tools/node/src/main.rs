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

//! `boson-node`: the Boson DHT node.
//!
//! The command tree is the same as the Java `boson-node`. Commands without a Rust implementation
//! yet are accepted and report that they are not implemented.

mod run;
mod shell;

use clap::{Args, Parser, Subcommand};

/// The Boson DHT node.
#[derive(Parser, Debug)]
#[command(name = "boson-node", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a Boson DHT node. `boson-node run --help` shows its options.
    #[command(disable_help_flag = true)]
    Run(Passthrough),
    /// Explore the DHT from an interactive shell.
    Shell(shell::Options),
    /// Configure a packaged bootstrap node.
    Setup(Passthrough),
    /// Show the routing table a node saved in its data directory.
    Cache(Passthrough),
    /// Show the id of the node a configuration file describes.
    Id(Passthrough),
    /// Write, show and check a node's configuration.
    #[command(subcommand)]
    Config(ConfigCommand),
}

/// Arguments handed on as they are: to a command with its own parser, or one not implemented yet.
#[derive(Args, Debug)]
struct Passthrough {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    args: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Write a node configuration, with a new node identity.
    Init(Passthrough),
    /// Show which configuration file is in effect, and what it sets.
    Show(Passthrough),
    /// Check that a configuration file is usable.
    Check(Passthrough),
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    match Cli::parse().command {
        Command::Run(passthrough) => {
            let args = std::iter::once("boson-node run".to_string()).chain(passthrough.args);
            run::run(args.collect()).await;
        }
        Command::Shell(options) => shell::run(options).await,
        Command::Setup(_) => pending("setup"),
        Command::Cache(_) => pending("cache"),
        Command::Id(_) => pending("id"),
        Command::Config(ConfigCommand::Init(_)) => pending("config init"),
        Command::Config(ConfigCommand::Show(_)) => pending("config show"),
        Command::Config(ConfigCommand::Check(_)) => pending("config check"),
    }
}

/// Reports a command of the Java `boson-node` that has no Rust implementation yet.
fn pending(command: &str) {
    eprintln!("boson-node {command}: not implemented yet");
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn command_tree_is_valid() {
        super::Cli::command().debug_assert();
    }
}
