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
            Command::new("add")
                .about("Send a friend request to specified user ID")
                .visible_alias("request")
                .arg(
                    Arg::new("USERID")
                        .required(true)
                        .help("User ID of the friend to add"),
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
                .about("List all friends (user IDs only)"),
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
        Some(("add" | "request", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            let greeting = subargs.get_one::<String>("greeting").map(|s| s.as_str());
            add_friend(client, user_id, greeting).await;
        }
        Some(("accept", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            accept_friend(client, user_id).await;
        }
        Some(("list", _)) => {
            list_friends(client).await;
        }
        Some(("info", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            show_info(client, user_id).await;
        }
        Some(("remove" | "delete", subargs)) => {
            let user_id = subargs.get_one::<String>("USERID").unwrap();
            remove_friend(client, user_id).await;
        }
        _ => {
            println!("Invalid friend command. Use 'friend --help' for usage information.");
        }
    }
}

async fn add_friend(client: &Arc<Client>, user_id_str: &str, greeting: Option<&str>) {
    let Some(target_id) = parse_id(user_id_str) else {
        return;
    };
    if target_id == *client.user_id() {
        println!("Cannot send friend request to yourself");
        return;
    }
    match client
        .friend_request(target_id, greeting.map(ToString::to_string))
        .await
    {
        Ok(()) => {
            if let Some(msg) = greeting {
                println!("Friend request sent to {target_id} with greeting: \"{msg}\"");
            } else {
                println!("Friend request sent to {target_id}");
            }
        }
        Err(e) => println!("Sending friend request failed: {e}"),
    }
}

async fn accept_friend(client: &Arc<Client>, user_id_str: &str) {
    let Some(target_id) = parse_id(user_id_str) else {
        return;
    };
    match client.accept_friend_request(&target_id).await {
        Ok(()) => println!("Accepted friend request from {target_id}"),
        Err(e) => println!("Accepting friend request failed: {e}"),
    }
}

async fn list_friends(client: &Arc<Client>) {
    match client.get_contacts().await {
        Ok(contacts) => {
            let mut count = 0;
            for contact in contacts.iter().filter(|c| {
                c.contact_type() == ContactType::Friend || c.contact_type() == ContactType::Auto
            }) {
                println!("{}", contact.id());
                count += 1;
            }
            if count == 0 {
                println!("No friends found");
            }
        }
        Err(e) => println!("Listing friends failed: {e}"),
    }
}

async fn show_info(client: &Arc<Client>, user_id_str: &str) {
    let Some(target_id) = parse_id(user_id_str) else {
        return;
    };

    match client.get_contact(&target_id).await {
        Ok(Some(contact)) => {
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
        }
        Ok(None) => match client.get_friend_request(&target_id).await {
            Ok(Some(req)) => {
                println!("User ID:      {}", req.initiator_id());
                println!("Status:       Friend Request Pending");
                println!("Greeting:     {}", req.hello().unwrap_or("<none>"));
                println!("Accepted:     {}", req.is_accepted());
                println!("Expired:      {}", req.is_expired());
            }
            _ => {
                println!("User {target_id} was not found in contacts");
            }
        },
        Err(e) => println!("Retrieving user details failed: {e}"),
    }
}

async fn remove_friend(client: &Arc<Client>, user_id_str: &str) {
    let Some(target_id) = parse_id(user_id_str) else {
        return;
    };
    match client.remove_contact(&target_id).await {
        Ok(()) => println!("Friend {target_id} removed"),
        Err(e) => println!("Removing friend failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_friend_cli() {
        let cmd = cli();

        // 1) friend add USERID [--greeting HELLO]
        let m = cmd
            .clone()
            .try_get_matches_from(["friend", "add", "user123", "--greeting", "hello"])
            .unwrap();
        assert_eq!(m.subcommand_name(), Some("add"));
        let sub_m = m.subcommand_matches("add").unwrap();
        assert_eq!(sub_m.get_one::<String>("USERID").unwrap(), "user123");
        assert_eq!(sub_m.get_one::<String>("greeting").unwrap(), "hello");

        // 2) friend accept USERID
        let m = cmd
            .clone()
            .try_get_matches_from(["friend", "accept", "user456"])
            .unwrap();
        assert_eq!(m.subcommand_name(), Some("accept"));
        let sub_m = m.subcommand_matches("accept").unwrap();
        assert_eq!(sub_m.get_one::<String>("USERID").unwrap(), "user456");

        // 3) friend list
        let m = cmd.clone().try_get_matches_from(["friend", "list"]).unwrap();
        assert_eq!(m.subcommand_name(), Some("list"));

        // 4) friend info USERID
        let m = cmd
            .clone()
            .try_get_matches_from(["friend", "info", "user789"])
            .unwrap();
        assert_eq!(m.subcommand_name(), Some("info"));
        let sub_m = m.subcommand_matches("info").unwrap();
        assert_eq!(sub_m.get_one::<String>("USERID").unwrap(), "user789");

        // 5) friend remove USERID
        let m = cmd
            .clone()
            .try_get_matches_from(["friend", "remove", "user999"])
            .unwrap();
        assert_eq!(m.subcommand_name(), Some("remove"));
        let sub_m = m.subcommand_matches("remove").unwrap();
        assert_eq!(sub_m.get_one::<String>("USERID").unwrap(), "user999");
    }
}
