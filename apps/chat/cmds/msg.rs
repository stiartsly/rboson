use std::sync::Arc;

use clap::{Arg, ArgMatches, Command};

use boson::messaging::Client;

use super::parse_id;

pub(crate) fn cli() -> Command {
    Command::new("msg")
        .about("Send a text message to a friend")
        .arg(
            Arg::new("CONTENT")
                .required(true)
                .help("Text message content"),
        )
        .arg(
            Arg::new("to")
                .long("to")
                .required(true)
                .value_name("USERID")
                .help("User ID of the recipient"),
        )
}

pub(crate) async fn execute(args: &ArgMatches, client: &Arc<Client>) {
    let content = args.get_one::<String>("CONTENT").unwrap();
    let user_id = args.get_one::<String>("to").unwrap();
    let Some(recipient) = parse_id(user_id) else {
        return;
    };

    match client.message(Some(recipient)).text_body(content).send().await {
        Ok(_) => println!("Message sent to {recipient}"),
        Err(error) => println!("Sending message failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_cli() {
        let matches = cli()
            .try_get_matches_from(["msg", "Hello Alice", "--to", "alice"])
            .unwrap();
        assert_eq!(matches.get_one::<String>("CONTENT").unwrap(), "Hello Alice");
        assert_eq!(matches.get_one::<String>("to").unwrap(), "alice");
    }
}
