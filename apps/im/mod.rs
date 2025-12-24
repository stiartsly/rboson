use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use clap::{error, Parser, ArgMatches, Command};
use reedline::{Reedline, Signal};

mod prompt;
use prompt::MyPrompt;

mod cmds {
    pub(crate) mod channel_cmd;
    pub(crate) mod device_cmd;
    pub(crate) mod info_cmd;
}

use boson::{
    configuration as cfg,
    signature,
    Id,
    dht::Node,
    appdata_store::AppDataStoreBuilder,
};

use boson::messaging::{
    UserProfile,
    MessagingClient,
    MessagingAgent,
    Message,
    ClientBuilder,
    Contact,
    ConnectionListener,
    MessageListener,
    ContactListener,
    ProfileListener,
};

fn build_cli() -> Command {
    let mut cmd = Command::new("tau")
        .about("Interactive messaging shell application")
        .no_binary_name(true)
        .subcommand_required(true)
        .subcommand(cmds::channel_cmd::channel_cli())
        .subcommand(cmds::device_cmd::device_cli())
        .subcommand(cmds::info_cmd::info_cli())
        .help_template("{subcommands}");
        //.override_help("My custom help message\n");

    cmd.error(error::ErrorKind::InvalidSubcommand, "Invalid command provided");
    cmd
}

async fn execute_command(matches: ArgMatches, client: &Arc<Mutex<MessagingClient>>) {
    match matches.subcommand() {
        Some(("channel", ch)) => match ch.subcommand() {
            Some(("create", m)) => {
                let name = m.get_one::<String>("NAME").unwrap();
                let notice: Option<String> = m.get_one::<String>("notice").cloned();
                println!(
                    "[OK] Channel created: name={:?} {}",
                    name,
                    notice.as_ref().map(|v| format!("--notice={v}")).unwrap_or("".to_string())
                );

                let rc = client.lock().unwrap().create_channel(
                    None,
                    name,
                    notice.as_deref()
                ).await;

                let channel = rc.unwrap();
                println!("Channel created id: {}", channel.id());
            }
            Some(("delete", m)) => {
                let id = m.get_one::<String>("ID").unwrap();
                let Ok(id) = Id::try_from(id.as_str()) else {
                    println!("Error: invalid channel id: {}", id);
                    return;
                };

                println!("Deleting channel: {}", id);
                _ = client.lock().unwrap().remove_channel(&id).await.map_err(|e| {
                    println!("Error deleting channel: {}", e);
                }).map(|_| {
                    println!("Channel {} is deleted.", id);
                });
            }

            Some(("join", m)) => {
                let ticket = m.get_one::<String>("TICKET").unwrap();
                println!("Joining channel with ticket: {}", ticket);
                /*
                _ = client.lock().unwrap().join_channel(ticket).await.map_err(|e| {
                    println!("Failed to join channel: {{{}}}", e);
                }).map(|_| {
                    println!("Joined channel with ticket: {}", ticket);
                });
                */
            }
            Some(("leave", m)) => {
                let id = m.get_one::<String>("ID").unwrap();
                let Ok(id) = Id::try_from(id.as_str()) else {
                    println!("Error: invalid channel id: {}", id);
                    return;
                };

                println!("Leaving a channel: {}", id);
                _ = client.lock().unwrap().leave_channel(&id).await.map_err(|e| {
                    println!("Error leaving channel: {}", e);
                }).map(|_| {
                    println!("Channel {} left.", id);
                });
            }

            Some(("ticket", m)) => {
                let id = m.get_one::<String>("ID").unwrap();
                let Ok(channel_id) = Id::try_from(id.as_str()) else {
                    println!("Error: invalid channel id: {}", id);
                    return;
                };

                let invitee = m.get_one::<String>("invitee");
                let invitee_id = match invitee {
                    Some(v) => {
                        let Ok(ii) = Id::try_from(v.as_str()) else {
                            println!("Error: invalid invitee id: {}", v);
                            return;
                        };
                        Some(ii)
                    }
                    None => None,
                };

                println!("Creating ticket for channel: {}", channel_id);
                let rc = client.lock().unwrap().create_invite_ticket(
                    &channel_id,
                    invitee_id.as_ref()
                ).await;

                match rc {
                    Ok(ticket) => {
                        println!("Channel ticket created: {}", ticket);
                    }
                    Err(e) => {
                        println!("Failed to create channel ticket: {{{}}}", e);
                    }
                }
            }

            Some(("info", m)) => {
                let id = m.get_one::<String>("ID").unwrap();
                let Ok(channel_id) = Id::try_from(id.as_str()) else {
                    println!("Error: invalid channel id: {}", id);
                    return;
                };

                let rc = client.lock().unwrap().channel(&channel_id).await;
                match rc {
                    Ok(Some(channel)) => {
                        println!("Channel info: {}", channel);
                    },
                    Ok(None) => {
                        println!("No channel found with id: {}", id);
                    },
                    Err(e) => {
                        println!("Failed to create channel ticket: {}", e);
                    }
                }
            }
            Some(("list", _m)) => {
                println!("[OK] Listing channels");
            }
            _ => {
                println!(">>>> Unknown channel subcommand");
            }
        },

        Some(("device", dv)) => match dv.subcommand() {
            Some(("list", m)) => {
                let all = m.get_flag("all");
                _ = client.lock().unwrap().devices().await.map_err(|e| {
                    println!("Error listing devices: {e}");
                }).map(|devices| {
                    println!("Devices (all:{},total:{}):", all, devices.len());
                    for device in devices {
                        println!("Device id({}) \n\tname({})\n\tapp({})\n\tcreated({:?})\n\tlast_seen({:?})\n\tlast_address({})",
                            device.id(),
                            device.name(),
                            device.app(),
                            device.created(),
                            device.last_seen(),
                            device.last_address()
                        );
                    }
                });
            }
            Some(("revoke", m)) => {
                let id = m.get_one::<String>("id").unwrap();
                let Ok(id) = Id::try_from(id.as_str()) else {
                    println!("Error: invalid device id {}", id);
                    return;
                };
                _ = client.lock().unwrap().revoke_device(&id).await.map_err(|e| {
                    println!("Error revoking device: {e}");
                }).map(|_| {
                    println!("Device {} is revoked.", id);
                });
            }
            _ => println!("Unknown device command"),
        },

        Some(("me", _)) => {
            println!("Show my information:");
            println!(" userid:\t{}", client.lock().unwrap().userid());
            println!(" deviceid:\t{}", client.lock().unwrap().deviceid());
            println!(" \nShow information of messaging service: ");
            println!(" peerid:\t{}", client.lock().unwrap().messaging_peer().id());
            println!(" nodeid:\t{}", client.lock().unwrap().messaging_peer().nodeid());
        }
        _ => println!("Unknown command"),
    }
}

#[derive(Parser, Debug)]
#[command(name = "tau")]
#[command(version = "1.0")]
#[command(about = "Tau iteractive messaging shell", long_about = None)]
struct Options {
    #[arg(short, long, value_name = "FILE")]
    config: String,

    #[arg(short='D', long)]
    daemonize: bool
}

#[tokio::main]
async fn main(){
    let opts = Options::parse();
    let cfg = cfg::Builder::new().load(&opts.config)
        .map_err(|e| panic!("{e}"))
        .unwrap()
        .build()
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let Some(ucfg) = cfg.user() else {
        eprintln!("User item is not found in config file");
        return;
    };

    let Some(dcfg) = cfg.device() else {
        eprintln!("Device item is not found in config file");
        return;
    };

    let Some(mcfg) = cfg.messaging() else {
        eprintln!("Messaging item not found in config file");
        return;
    };

    let peerid = Id::try_from(mcfg.server_peerid())
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let result = Node::new(&cfg);
    if let Err(e) = result {
        eprintln!("Creating boson Node instance error: {e}");
        return;
    }

    let node = Arc::new(Mutex::new(result.unwrap()));
    node.lock().unwrap().start();

    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut path = String::new();
    path.push_str(cfg.data_dir());
    path.push_str("/messaging.cache");

    let mut appdata_store = AppDataStoreBuilder::new("im")
        .with_path(path.as_str())
        .with_node(&node)
        .with_peerid(&peerid)
        .build()
        .unwrap();

    if let Err(e) = appdata_store.load().await {
        eprintln!("Loading app data store error: {e}");
        node.lock().unwrap().stop();
        return;
    }

    let Some(peer) = appdata_store.service_peer() else {
        println!("Messaging peer is not found!!!, please run it later.");
        node.lock().unwrap().stop();
        return;
    };

    let Some(_ni) = appdata_store.service_node() else {
        eprintln!("Node hosting the peer not found!!!");
        node.lock().unwrap().stop();
        return;
    };

    //println!("Messaging Peer: {}", peer);
    //println!("Messaging Node: {}", ni);

    let usk: signature::PrivateKey = match ucfg.private_key().try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Failed to convert private key from hex format");
            node.lock().unwrap().stop();
            return;
        }
    };

    let dsk: signature::PrivateKey = match dcfg.private_key().try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Failed to convert device private key from hex format");
            node.lock().unwrap().stop();
            return;
        }
    };

    let user_key = signature::KeyPair::from(&usk);
    let device_key = signature::KeyPair::from(&dsk);

    let result = ClientBuilder::new()
        .with_user_key(user_key)
        .with_user_name(ucfg.name().unwrap_or("guest"))
        .with_device_key(device_key)
        .with_device_name("test-device")
        .with_device_node(node.clone())
        .with_app_name("test-im")
        .with_messaging_peer(peer.clone()).unwrap()
        .with_messaging_repository("test-repo")
        .register_user_and_device(ucfg.password().map_or("secret", |v|v))
        .with_connection_listener(ConnectionListenerTest)
        .with_message_listener(MessageListenerTest)
        .with_contact_listener(ContactListenerTest)
        .with_profile_listener(ProfileListenerTest)
        .build_into()
        .await;

    let mut client = match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Creating messaging client instance error: {}", e);
            node.lock().unwrap().stop();
            return;
        }
    };

    let rc = client.start().await;
    thread::sleep(Duration::from_secs(1));
    if let Err(e) = rc {
        eprintln!("Starting messaging client error: {{{e}}}");
        node.lock().unwrap().stop();
        return;
    }

    let rc = client.connect().await;
    if let Err(e) = rc {
        eprintln!("Connecting to messaging service error: {{{e}}}");
        _ = client.stop(true).await;
        node.lock().unwrap().stop();
        return;
    }

    let client = Arc::new(Mutex::new(client));
    let mut cli = build_cli();
    let mut rl = Reedline::create();
    let prompt = MyPrompt;

    println!("Welcome to interactive messaging shell. Type 'exit' to quit.\n");

    loop {
        let Ok(sig) = rl.read_line(&prompt) else {
            println!("\n Fatal error occurred.");
            continue;
        };
        match sig {
            Signal::Success(line) => {
                let input = line.trim();

                if input.is_empty() {
                    continue;
                }

                match input {
                    "exit" | "quit" => {
                        println!("Goodbye!");
                        break;
                    },
                    "help" => {
                        _ = cli.print_long_help();
                        continue;
                    }
                    _ => {}
                }

                let args: Vec<String> = input.split_whitespace().map(|s| s.to_string())
                    .collect();

                match args[0].as_str() {
                    "help" => {
                        _ = match cli.find_subcommand_mut(args[1].as_str()) {
                            Some(cmd) => cmd.print_long_help(),
                            None => cli.print_long_help(),
                        };
                        continue;
                    }
                    _ => {}
                }

                let cmd = args.join(" ");
                match cli.clone().try_get_matches_from(args) {
                    Ok(matches) => execute_command(matches, &client).await,
                    Err(_) => {
                        println!("Error: command not found: '{}'", cmd);
                    }
                }
            }
            Signal::CtrlC | Signal::CtrlD => {
                println!("\nGoodbye!");
                break;
            }
        }
    }

    _ = client.lock().unwrap().stop(true).await;
    node.lock().unwrap().stop();
}

struct ConnectionListenerTest;
impl ConnectionListener for ConnectionListenerTest {
    fn on_connecting(&self) {
        println!("Connecting to messaging service...");
    }

    fn on_connected(&self) {
       // println!("Connected to messaging service");
    }

    fn on_disconnected(&self) {
        println!("Disconnected from messaging service");
    }
}

struct MessageListenerTest;
impl MessageListener for MessageListenerTest {
    fn on_message(&self, message: &Message) {
        println!("Received message: {:?}", message);
    }
    fn on_sending(&self, message: &Message) {
        println!("Sending message: {:?}", message);
    }

    fn on_sent(&self, message: &Message) {
        println!("Message sent: {:?}", message);
    }

    fn on_broadcast(&self, message: &Message) {
        println!("Broadcast message: {:?}", message);
    }
}

struct ContactListenerTest;
impl ContactListener for ContactListenerTest {
    fn on_contacts_updating(&self,
        _version_id: &str,
        _contacts: Vec<Contact>
    ) {
        println!("Contacts updating!");
    }

    fn on_contacts_updated(&self,
        _base_version_id: &str,
        _new_version_id: &str,
        _contacts: Vec<Contact>
    ) {
        println!("Contacts updated");
    }

    fn on_contacts_cleared(&self) {
        println!("Contacts cleared");
    }

    fn on_contact_profile(&self,
        _contact_id: &Id,
        _profile: &Contact
    ) {
        println!("Contact profile ");
    }
}

struct ProfileListenerTest;
impl ProfileListener for ProfileListenerTest {
    fn on_user_profile_acquired(&self, _profile: &UserProfile) {
        println!("User profile acquired");
    }

    fn on_user_profile_changed(&self, _name: &str, _avatar: bool) {
        println!("User profile changed");
    }
}
