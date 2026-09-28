use std::sync::Arc;
use clap::{Arg, ArgMatches, Command};
use boson::messaging::{Client, ContactType};
use super::parse_id;

pub(crate) fn cli() -> Command {
    Command::new("friend")
        .about("Manage friends and friend requests")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("request")
                .about("Send a friend request to specified user ID")
                .arg(
                    Arg::new("USERID")
                        .required(true)
                        .help("User ID of the friend to send a request"),
                )
                .arg(
                    Arg::new("greeting")
                        .short('g')
                        .long("greeting")
                        .value_name("HELLO")
                        .help("Optional greeting message"),
                ),
        )
        .subcommand(
            Command::new("accept")
                .about("Accept an incoming friend request from user ID")
                .arg(
                    Arg::new("USERID")
                        .required(true)
                        .help("User ID of the friend to accept"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List all friends (user IDs only)")
                .arg(
                    Arg::new("request")
                        .long("request")
                        .help("Show pending friend requests"),
                ),
        )
        .subcommand(
            Command::new("info")
                .about("Show details for specified user ID")
                .arg(
                    Arg::new("USERID")
                        .required(true)
                        .help("User ID to show details for"),
                ),
        )
        .subcommand(
            Command::new("remove")
                .about("Remove a friend by user ID")
                .visible_alias("delete")
                .arg(
                    Arg::new("USERID")
                        .required(true)
                        .help("User ID of the friend to remove"),
                ),
        )
}

pub(crate) async fn execute(args: &ArgMatches, client: &Arc<Client>) {
    match args.subcommand() {
        Some(("request", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            let greeting = subargs.get_one::<String>("greeting").map(|s| s.as_str());
            friend_request(client, user_id, greeting).await;
        }
        Some(("accept", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            accept_friend(client, user_id).await;
        }
        Some(("list", _subargs)) => {
            list_friends(client).await;
        }
        Some(("info", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            show_info(client, user_id).await;
        }
        Some(("delete", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            remove_friend(client, user_id).await;
        }
        _ => {
            println!("Invalid friend command. Use 'friend --help' for usage information.");
        }
    }
}

async fn friend_request(client: &Arc<Client>, user_idstr: &str, greeting: Option<&str>) {
    let Some(userid) = parse_id(user_idstr) else {
        return;
    };
    if userid == *client.user_id() {
        println!("Cannot send friend request to yourself");
        return;
    }
    if let Err(e) = client
        .friend_request(userid, greeting.map(ToString::to_string))
        .await {
        println!("Sending friend request failed: {e}");
        return;
    }

    if let Some(msg) = greeting {
        println!("Friend request sent to {userid} with greeting: \"{msg}\"");
    } else {
        println!("Friend request sent to {userid} without a greeting");
    }
}

async fn accept_friend(client: &Arc<Client>, user_idstr: &str) {
    let Some(userid) = parse_id(user_idstr) else {
        return;
    };
    match client.accept_friend_request(&userid).await {
        Ok(()) => println!("Accepted friend request from {userid}"),
        Err(e) => println!("Accepting friend request failed: {e}"),
    }
}

async fn list_friends(client: &Arc<Client>) {
    let contacts = match client.get_contacts().await {
        Ok(v) => v,
        Err(e) => {
            println!("Listing friends failed: {e}");
            return;
        }
    };

    let mut count = 0;
    for contact in contacts.iter().filter(|c| {
        c.contact_type() == ContactType::Friend ||
        c.contact_type() == ContactType::Auto
    }) {
        println!("{}", contact.id());
        count += 1;
    }
    if count == 0 {
        println!("No friends found");
    }
}

async fn show_info(client: &Arc<Client>, user_idstr: &str) {
    let Some(userid) = parse_id(user_idstr) else {
        return;
    };

    let contact = match client.get_contact(&userid).await {
        Ok(v) => v,
        Err(e) => {
            println!("Retrieving user details failed: {e}");
            return;
        }
    };

    if let Some(contact) = contact {
        println!("User ID:      {}", contact.id());
        println!("Name:         {}", contact.name().unwrap_or("<none>"));
        println!("Remark:       {}", contact.remark().unwrap_or("<none>"));
        println!("Type:         {:?}", contact.contact_type());
        println!("Tags:         {}", contact.tags().unwrap_or("<none>"));
        println!("Muted:        {}", contact.is_muted());
        println!("Blocked:      {}", contact.is_blocked());
        println!("Revision:     {}", contact.revision());
        println!("Created At:   {}", contact.created_at());
        println!("Updated At:   {}", contact.updated_at());
        return;
    }

    match client.get_friend_request(&userid).await {
        Ok(Some(req)) => {
            println!("User ID:      {}", req.initiator_id());
            println!("Status:       Friend Request Pending");
            println!("Greeting:     {}", req.hello().unwrap_or("<none>"));
            println!("Accepted:     {}", req.is_accepted());
            println!("Expired:      {}", req.is_expired());
        }
        _ => {
            println!("User {userid} was not found in contacts");
        }
    }
}

async fn remove_friend(client: &Arc<Client>, user_idstr: &str) {
    let Some(userid) = parse_id(user_idstr) else {
        return;
    };
    match client.remove_contact(&userid).await {
        Ok(()) => println!("Friend {userid} removed"),
        Err(e) => println!("Removing friend failed: {e}"),
    }
}
