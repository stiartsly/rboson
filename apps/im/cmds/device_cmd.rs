
use clap::{Command, Arg, ArgAction};

pub(crate) fn device_cli() -> Command {
    Command::new("device")
        .about("Manage devices")
        .subcommand(
            Command::new("list")
                .about("List devices")
                .arg(
                    Arg::new("all")
                        .long("all")
                        .help("List all devices")
                        .action(ArgAction::SetTrue),
                )
        )
        .subcommand(
            Command::new("revoke")
                .about("Revoke a device")
                .arg(
                    Arg::new("id")
                        .required(true)
                        .help("Device ID"),
                )
        )
        .help_template("{subcommands}")
}
