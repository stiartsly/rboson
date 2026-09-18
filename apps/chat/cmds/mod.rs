use std::sync::Arc;

use clap::{error, ArgMatches, Command};

use boson::{messaging::MessagingClient, Id};

pub(crate) mod channel;
pub(crate) mod device;
pub(crate) mod me;

pub(crate) fn build_cli() -> Command {
    let mut command = Command::new("photon")
        .about("Interactive messaging shell application")
        .no_binary_name(true)
        .subcommand_required(true)
        .subcommand(channel::cli())
        .subcommand(device::cli())
        .subcommand(me::cli())
        .help_template("{subcommands}");
    command.error(
        error::ErrorKind::InvalidSubcommand,
        "Invalid command provided",
    );
    command
}

pub(crate) async fn execute_command(matches: ArgMatches, client: &Arc<dyn MessagingClient>) {
    match matches.subcommand() {
        Some(("channel", channel)) => channel::execute(channel, client).await,
        Some(("device", device)) => device::execute(device, client).await,
        Some(("me", me)) => me::execute(me, client).await,
        _ => println!("Unknown command"),
    }
}

pub(crate) fn parse_id(value: &str) -> Option<Id> {
    match Id::try_from(value) {
        Ok(id) => Some(id),
        Err(error) => {
            println!("Invalid id: {error}");
            None
        }
    }
}
