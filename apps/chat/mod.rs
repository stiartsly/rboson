use std::{collections::HashMap, fs::File, str::FromStr, sync::Arc};

use clap::{error, ArgMatches, Command, Parser};
use reedline::{Reedline, Signal};

use boson::{
    messaging::{
        Channel, ChannelListener, Configuration, ConnectionListener, Contact, ContactListener,
        ContactType, FriendRequestListener, InviteTicket, Message, MessageListener,
        MessagingClient, MessagingClientBuilder, Permission, SessionInfo, SessionListener,
    },
    Id,
};

mod prompt;
use prompt::MyPrompt;

mod cmds {
    pub(crate) mod channel_cmd;
    pub(crate) mod device_cmd;
    pub(crate) mod info_cmd;
}

fn build_cli() -> Command {
    let mut command = Command::new("tau")
        .about("Interactive messaging shell application")
        .no_binary_name(true)
        .subcommand_required(true)
        .subcommand(cmds::channel_cmd::channel_cli())
        .subcommand(cmds::device_cmd::device_cli())
        .subcommand(cmds::info_cmd::info_cli())
        .help_template("{subcommands}");
    command.error(
        error::ErrorKind::InvalidSubcommand,
        "Invalid command provided",
    );
    command
}

async fn execute_command(matches: ArgMatches, client: &Arc<dyn MessagingClient>) {
    match matches.subcommand() {
        Some(("channel", channel)) => match channel.subcommand() {
            Some(("create", args)) => {
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
                    Err(error) => eprintln!("Creating channel failed: {error}"),
                }
            }
            Some(("delete", args)) => {
                let Some(id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
                    return;
                };
                match client.remove_channel(&id).await {
                    Ok(()) => println!("Channel {id} deleted"),
                    Err(error) => eprintln!("Deleting channel failed: {error}"),
                }
            }
            Some(("join", args)) => {
                let encoded = args.get_one::<String>("TICKET").unwrap();
                let ticket = match InviteTicket::from_str(encoded) {
                    Ok(ticket) => ticket,
                    Err(error) => {
                        eprintln!("Invalid invite ticket: {error}");
                        return;
                    }
                };
                match client.join_channel(ticket).await {
                    Ok(channel) => println!("Joined channel {}", channel.id()),
                    Err(error) => eprintln!("Joining channel failed: {error}"),
                }
            }
            Some(("leave", args)) => {
                let Some(id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
                    return;
                };
                match client.leave_channel(&id).await {
                    Ok(()) => println!("Left channel {id}"),
                    Err(error) => eprintln!("Leaving channel failed: {error}"),
                }
            }
            Some(("ticket", args)) => {
                let Some(channel_id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
                    return;
                };
                let invitee = match args.get_one::<String>("invitee") {
                    Some(value) => match Id::try_from(value.as_str()) {
                        Ok(id) => Some(id),
                        Err(error) => {
                            eprintln!("Invalid invitee id: {error}");
                            return;
                        }
                    },
                    None => None,
                };
                match client.create_invite_ticket(&channel_id, invitee).await {
                    Ok(ticket) => println!("Invite ticket: {ticket}"),
                    Err(error) => eprintln!("Creating invite ticket failed: {error}"),
                }
            }
            Some(("info", args)) => {
                let Some(id) = parse_id(args.get_one::<String>("ID").unwrap()) else {
                    return;
                };
                match client.get_contact(&id).await {
                    Ok(Some(contact)) if contact.contact_type() == ContactType::Channel => {
                        print_contact(contact.as_ref())
                    }
                    Ok(Some(_)) => println!("{id} is not a channel"),
                    Ok(None) => println!("Channel {id} was not found"),
                    Err(error) => eprintln!("Retrieving channel failed: {error}"),
                }
            }
            Some(("list", _)) => match client.get_contacts().await {
                Ok(contacts) => {
                    for contact in contacts
                        .iter()
                        .filter(|contact| contact.contact_type() == ContactType::Channel)
                    {
                        print_contact(contact.as_ref());
                    }
                }
                Err(error) => eprintln!("Listing channels failed: {error}"),
            },
            _ => eprintln!("Unknown channel command"),
        },
        Some(("device", device)) => match device.subcommand() {
            Some(("list", _)) => match client.get_sessions().await {
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
                Err(error) => eprintln!("Listing sessions failed: {error}"),
            },
            Some(("revoke", args)) => {
                let Some(id) = parse_id(args.get_one::<String>("id").unwrap()) else {
                    return;
                };
                match client.revoke_session(&id).await {
                    Ok(()) => println!("Session {id} revoked"),
                    Err(error) => eprintln!("Revoking session failed: {error}"),
                }
            }
            _ => eprintln!("Unknown device command"),
        },
        Some(("me", _)) => {
            println!("user id:    {}", client.user_id());
            println!("device id:  {}", client.device_id());
            println!("service id: {}", client.service_peer_id());
            println!(
                "endpoint:   {}",
                client
                    .service_endpoint()
                    .unwrap_or("<DHT discovery required>")
            );
            println!(
                "running={} connected={} ready={}",
                client.is_running(),
                client.is_connected(),
                client.is_ready()
            );
        }
        _ => eprintln!("Unknown command"),
    }
}

fn parse_id(value: &str) -> Option<Id> {
    match Id::try_from(value) {
        Ok(id) => Some(id),
        Err(error) => {
            eprintln!("Invalid id: {error}");
            None
        }
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

#[derive(Parser, Debug)]
#[command(name = "chat", version = "1.0", about = "Boson messaging chat")]
struct Options {
    #[arg(short, long, value_name = "FILE")]
    config: String,

    #[arg(long)]
    shadow: bool,

    #[arg(short = 'D', long)]
    daemonize: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("chat: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse();
    if options.shadow {
        eprintln!("Note: --shadow is no longer needed by the updated messaging API");
    }
    if options.daemonize {
        eprintln!("Note: --daemonize does not detach the interactive shell");
    }

    let file = File::open(&options.config)?;
    let values: HashMap<String, serde_json::Value> = serde_yaml::from_reader(file)?;
    let config = Configuration::from_map(&values)?;
    let client = MessagingClientBuilder::new()
        .configuration(config)
        .connection_listener(Arc::new(ConsoleConnectionListener))
        .message_listener(Arc::new(ConsoleMessageListener))
        .channel_listener(Arc::new(ConsoleChannelListener))
        .contact_listener(Arc::new(ConsoleContactListener))
        .session_listener(Arc::new(ConsoleSessionListener))
        .friend_request_listener(Arc::new(ConsoleFriendRequestListener))
        .build()?;

    client.start().await?;

    let mut cli = build_cli();
    let mut editor = Reedline::create();
    let prompt = MyPrompt;
    println!("Welcome to the messaging shell. Type 'help' or 'exit'.");

    loop {
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let args: Vec<String> = line.split_whitespace().map(ToString::to_string).collect();
                if args.is_empty() {
                    continue;
                }
                if matches!(args[0].as_str(), "exit" | "quit") {
                    break;
                }
                if args[0] == "help" {
                    if let Some(name) = args.get(1) {
                        match cli.find_subcommand_mut(name) {
                            Some(command) => command.print_long_help()?,
                            None => cli.print_long_help()?,
                        }
                    } else {
                        cli.print_long_help()?;
                    }
                    println!();
                    continue;
                }
                match cli.clone().try_get_matches_from(args) {
                    Ok(matches) => execute_command(matches, &client).await,
                    Err(error) => eprintln!("{error}"),
                }
            }
            Ok(Signal::CtrlC | Signal::CtrlD) => break,
            Ok(_) => continue,
            Err(error) => {
                eprintln!("Input error: {error}");
                break;
            }
        }
    }

    client.stop().await?;
    Ok(())
}

struct ConsoleConnectionListener;

impl ConnectionListener for ConsoleConnectionListener {
    fn on_connecting(&self) {
        println!("Connecting to messaging service...");
    }

    fn on_connected(&self) {
        println!("Connected to messaging service");
    }

    fn on_ready(&self) {
        println!("Messaging service is ready");
    }

    fn on_disconnected(&self) {
        println!("Disconnected from messaging service");
    }
}

struct ConsoleMessageListener;

impl MessageListener for ConsoleMessageListener {
    fn on_message(&self, message: &dyn Message) {
        println!("Received message {}", message.id());
    }

    fn on_sent(&self, message: &dyn Message) {
        println!("Sent message {}", message.id());
    }
}

struct ConsoleContactListener;

impl ContactListener for ConsoleContactListener {
    fn on_contact_added(&self, contact: &dyn Contact) {
        println!("Contact added: {}", contact.id());
    }

    fn on_contacts_updated(&self, contacts: &[Box<dyn Contact>]) {
        println!("Updated {} contact(s)", contacts.len());
    }

    fn on_contacts_removed(&self, contact_ids: &[Id]) {
        println!("Removed {} contact(s)", contact_ids.len());
    }

    fn on_contacts_cleared(&self) {
        println!("Contacts cleared");
    }
}

struct ConsoleChannelListener;

impl ChannelListener for ConsoleChannelListener {
    fn on_channel_created(&self, channel: &dyn Channel) {
        println!("Channel created: {}", channel.id());
    }

    fn on_channel_deleted(&self, channel: &dyn Channel) {
        println!("Channel deleted: {}", channel.id());
    }

    fn on_joined_channel(&self, channel: &dyn Channel) {
        println!("Joined channel: {}", channel.id());
    }

    fn on_left_channel(&self, channel: &dyn Channel) {
        println!("Left channel: {}", channel.id());
    }
}

struct ConsoleSessionListener;

impl SessionListener for ConsoleSessionListener {
    fn on_new_session(&self, session: &SessionInfo) {
        println!("New device session: {}", session.device_id());
    }
}

struct ConsoleFriendRequestListener;

impl FriendRequestListener for ConsoleFriendRequestListener {
    fn on_friend_request(&self, user_id: &Id, hello: Option<&str>) {
        println!(
            "Friend request from {}: {}",
            user_id,
            hello.unwrap_or("<no greeting>")
        );
    }

    fn on_friend_request_accepted(&self, user_id: &Id) {
        println!("Friend request accepted by {user_id}");
    }
}
