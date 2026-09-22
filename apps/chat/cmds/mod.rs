use std::sync::{Arc, Mutex};

use clap::{error, ArgMatches, Command};

use boson::{messaging::Client, Id};

macro_rules! println {
    () => {
        $crate::cmds::print_result(String::new())
    };
    ($($arg:tt)*) => {
        $crate::cmds::print_result(format!($($arg)*))
    };
}

type OutputHandler = Arc<dyn Fn(String) + Send + Sync>;

static RESULT_OUTPUT: Mutex<Option<OutputHandler>> = Mutex::new(None);

pub(crate) fn set_result_output(handler: impl Fn(String) + Send + Sync + 'static) {
    *RESULT_OUTPUT.lock().unwrap() = Some(Arc::new(handler));
}

pub(crate) fn print_result(line: String) {
    let handler = RESULT_OUTPUT.lock().unwrap().clone();
    if let Some(handler) = handler {
        handler(line);
    } else {
        std::println!("{line}");
    }
}

pub(crate) mod channel;
pub(crate) mod device;
pub(crate) mod friend;
pub(crate) mod info;
pub(crate) mod me;

pub(crate) fn build_cli() -> Command {
    let mut command = Command::new("photon")
        .about("Interactive messaging shell application")
        .no_binary_name(true)
        .subcommand_required(true)
        .subcommand(channel::cli())
        .subcommand(device::cli())
        .subcommand(friend::cli())
        .subcommand(info::cli())
        .subcommand(me::cli())
        .help_template("{subcommands}");
    command.error(
        error::ErrorKind::InvalidSubcommand,
        "Invalid command provided",
    );
    command
}

pub(crate) async fn execute_command(matches: ArgMatches, client: &Arc<Client>) {
    match matches.subcommand() {
        Some(("channel", channel)) => channel::execute(channel, client).await,
        Some(("device", device)) => device::execute(device, client).await,
        Some(("friend", friend)) => friend::execute(friend, client).await,
        Some(("info", info)) => info::execute(info, client).await,
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
