use std::{str::FromStr, sync::Arc};

use clap::{arg, Arg, ArgMatches, Command};

use boson::{
    messaging::{Client, Contact, ContactType, InviteTicket, MessagingClient, Permission},
    Id,
};

use super::parse_id;

pub(crate) fn cli() -> Command {
    Command::new("channel")
        .about("Manage channels")
        .subcommand_required(true)
        //.arg(arg!(-h --help "Print help information"))
        .subcommand(
            Command::new("create")
                .about("Create a channel")
                .arg(arg!(<NAME> "The channel name to create"))
                .arg(arg!(--notice <NOTICE> "The channel notice").required(false))
                .arg(
                    Arg::new("allow-inviter")
                        .short('i')
                        .long("allow-inviter")
                        .visible_alias("allow-invitor")
                        .help("Who may invite new channel members")
                        .default_value("owner")
                        .value_parser(["free", "member", "moderator", "owner"]),
                ),
        )
        .subcommand(
            Command::new("delete")
                .about("Delete channel")
                .arg(arg!(<ID> "The channel id to be removed"))
                .arg_required_else_help(true),
        )
        .subcommand(Command::new("list").about("List all channels"))
        .subcommand(
            Command::new("join")
                .about("Join a channel")
                .arg(arg!(<TICKET> "The invitation ticket used to join the channel")),
        )
        .subcommand(
            Command::new("leave")
                .about("Leave a channel")
                .arg(arg!(<ID> "The channel id to leave")),
        )
        .subcommand(
            Command::new("info")
                .about("Retrieve channel information")
                .arg(arg!(<ID> "The channel id to retrieve information for")),
        )
        .subcommand(
            Command::new("ticket")
                .about("Create a ticket")
                .arg(arg!(<ID> "The channel id to join on which the ticket is created"))
                .arg(arg!(--invitee <ID> "The invitee id").required(false)),
        )
        .help_template("{subcommands}")
        .disable_help_flag(true)
}

pub(crate) async fn execute(args: &ArgMatches, client: &Arc<Client>) {
    match args.subcommand() {
        Some(("create", args)) => create(args, client).await,
        Some(("delete", args)) => delete(args, client).await,
        Some(("join", args)) => join(args, client).await,
        Some(("leave", args)) => leave(args, client).await,
        Some(("ticket", args)) => ticket(args, client).await,
        Some(("info", args)) => info(args, client).await,
        Some(("list", _)) => list(client).await,
        _ => println!("Unknown channel command"),
    }
}

async fn create(args: &ArgMatches, client: &Arc<Client>) {
    let permission = match args
        .get_one::<String>("allow-inviter")
        .map(String::as_str)
        .unwrap_or("owner")
    {
        "free" => Permission::Public,
        "member" => Permission::MemberInvite,
        "moderator" => Permission::ModeratorInvite,
        _ => Permission::OwnerInvite,
    };
    let name = args.get_one::<String>("NAME").unwrap().clone();
    let notice = args.get_one::<String>("notice").cloned();
    match client.create_channel(permission, name, notice, None).await {
        Ok(channel) => println!("Channel created: {}", channel.id()),
        Err(error) => println!("Creating channel failed: {error}"),
    }
}

async fn delete(args: &ArgMatches, client: &Arc<Client>) {
    let Some(id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
        return;
    };
    match client.remove_channel(&id).await {
        Ok(()) => println!("Channel {id} deleted"),
        Err(error) => println!("Deleting channel failed: {error}"),
    }
}

async fn join(args: &ArgMatches, client: &Arc<Client>) {
    let encoded = args.get_one::<String>("TICKET").unwrap();
    let ticket = match InviteTicket::from_str(encoded) {
        Ok(ticket) => ticket,
        Err(error) => {
            println!("Invalid invite ticket: {error}");
            return;
        }
    };
    match client.join_channel(ticket).await {
        Ok(channel) => println!("Joined channel {}", channel.id()),
        Err(error) => println!("Joining channel failed: {error}"),
    }
}

async fn leave(args: &ArgMatches, client: &Arc<Client>) {
    let Some(id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
        return;
    };
    match client.leave_channel(&id).await {
        Ok(()) => println!("Left channel {id}"),
        Err(error) => println!("Leaving channel failed: {error}"),
    }
}

async fn ticket(args: &ArgMatches, client: &Arc<Client>) {
    let Some(channel_id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
        return;
    };
    let invitee = match args.get_one::<String>("invitee") {
        Some(value) => match Id::try_from(value.as_str()) {
            Ok(id) => Some(id),
            Err(error) => {
                println!("Invalid invitee id: {error}");
                return;
            }
        },
        None => None,
    };
    match client.create_invite_ticket(&channel_id, invitee).await {
        Ok(ticket) => println!("Invite ticket: {ticket}"),
        Err(error) => println!("Creating invite ticket failed: {error}"),
    }
}

async fn info(args: &ArgMatches, client: &Arc<Client>) {
    let Some(id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
        return;
    };
    match client.get_contact(&id).await {
        Ok(Some(contact)) if contact.contact_type() == ContactType::Channel => {
            print_contact(contact.as_ref())
        }
        Ok(Some(_)) => println!("{id} is not a channel"),
        Ok(None) => println!("Channel {id} was not found"),
        Err(error) => println!("Retrieving channel failed: {error}"),
    }
}

async fn list(client: &Arc<Client>) {
    match client.get_contacts().await {
        Ok(contacts) => {
            for contact in contacts
                .iter()
                .filter(|contact| contact.contact_type() == ContactType::Channel)
            {
                print_contact(contact.as_ref());
            }
        }
        Err(error) => println!("Listing channels failed: {error}"),
    }
}

fn print_contact(contact: &dyn Contact) {
    println!(
        "{} name={} muted={} blocked={}",
        contact.id(),
        contact.name().unwrap_or("-"),
        contact.is_muted(),
        contact.is_blocked()
    );
}
