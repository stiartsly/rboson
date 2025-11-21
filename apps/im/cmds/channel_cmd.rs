use clap::{arg, Command};

pub(crate) fn channel_cli() -> Command {
    Command::new("channel")
        .about("Manage channels")
        .subcommand_required(true)
        //.arg(arg!(-h --help "Print help information"))
        .subcommand(
            Command::new("create")
                .about("Create a channel")
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
                .arg(arg!(<ID> "The channel id to be removed"))
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("list")
                .about("List all channels")
        )
        .subcommand(
            Command::new("join")
                .about("Join a channel")
                .arg(arg!(<TICKET> "The invitation ticket used to join the channel"))
        )
        .subcommand(
            Command::new("leave")
                .about("Leave a channel")
                .arg(arg!(<ID> "The channel id to leave"))
        )
        .subcommand(
            Command::new("info")
                .about("Retrieve channel information")
                .arg(arg!(<ID> "The channel id to retrieve information for"))
        )
        .subcommand(
            Command::new("ticket")
                .about("Create a ticket")
                .arg(arg!(<ID> "The channel id to join for which the ticket is created"))
                .arg(arg!(--invitee <ID> "The invitee id").required(false))
        )
        .help_template("{subcommands}")
        .disable_help_flag(true)
}
