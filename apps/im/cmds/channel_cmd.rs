use clap::{arg, Command};

pub(crate) fn channel_cli() -> Command {
    Command::new("channel")
        .about("Manage channels")
        .subcommand_required(true)
        .arg(arg!(-h --help "Print help information"))
        .subcommand(
            Command::new("create")
                .about("Create a channel")
                //.allow_missing_positional(true)
                .arg(arg!(<NAME> "The channel name to create"))
                .arg(arg!(--notice <NOTICE> "The channel notice").required(false))
                .arg(arg!(-i --"allow-invitor" <WHO> "The permission for invite creator")
                        .required(false)
                        .default_value("owner")
                        .value_parser(["free", "member", "moderator", "owner"]),
                )
        )
        .subcommand(
            Command::new("delete")
                .about("Delete channel")
                .arg(arg!(<ID> "The channel id to delete"))
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("info")
                .about("Retrieve channel information")
                .arg(arg!(<ID> "The channel id to retrieve information for"))
        )
        .help_template("{subcommands}")
        .disable_help_flag(true)
        //.ignore_errors(true)
}
