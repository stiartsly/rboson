
use clap::Command;

pub(crate) fn info_cli() -> Command {
    Command::new("me")
        .about("Show client information")
        .help_template("{subcommands}")
}
