use std::sync::Arc;

use clap::{Arg, ArgAction, ArgMatches, Command};

use boson::messaging::{MessagingClient, Client};

use super::parse_id;

pub(crate) fn cli() -> Command {
    Command::new("device")
        .about("Manage devices")
        .subcommand(
            Command::new("list").about("List devices").arg(
                Arg::new("all")
                    .long("all")
                    .help("List all devices")
                    .action(ArgAction::SetTrue),
            ),
        )
        .subcommand(
            Command::new("revoke")
                .about("Revoke a device")
                .arg(Arg::new("id").required(true).help("Device ID")),
        )
        .help_template("{subcommands}")
}

pub(crate) async fn execute(args: &ArgMatches, client: &Arc<Client>) {
    match args.subcommand() {
        Some(("list", _)) => list(client).await,
        Some(("revoke", args)) => revoke(args, client).await,
        _ => println!("Unknown device command"),
    }
}

async fn list(client: &Arc<Client>) {
    match client.get_sessions().await {
        Ok(sessions) => {
            for session in sessions {
                println!(
                    "{} online={} last_active={} address={}",
                    session.device_id(),
                    session.is_online(),
                    session.last_active_ms(),
                    session.last_address().unwrap_or("-")
                );
            }
        }
        Err(error) => println!("Listing sessions failed: {error}"),
    }
}

async fn revoke(args: &ArgMatches, client: &Arc<Client>) {
    let Some(id) = parse_id(args.get_one::<String>("id").unwrap()) else {
        return;
    };
    match client.revoke_session(&id).await {
        Ok(()) => println!("Session {id} revoked"),
        Err(error) => println!("Revoking session failed: {error}"),
    }
}
