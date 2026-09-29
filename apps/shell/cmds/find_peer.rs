use boson::dht::Node;
use clap::{arg, value_parser, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("findpeer")
        .about("Look up peers announced under an id")
        .arg(arg!(<ID> "Target peer id (base58)"))
        .arg(
            arg!(-c --count <COUNT> "Expected number of peers")
                .default_value("8")
                .value_parser(value_parser!(usize)),
        )
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let id_str = matches.get_one::<String>("ID").unwrap();
    let Ok(peerid) = id_str.parse() else {
        red_print!("Invalid id '{id_str}'");
        return;
    };

    let count = *matches.get_one::<usize>("count").unwrap();
    println!("Attempting to find peers with id: {peerid} ...");
    let peer = match node.find_peer(&peerid, -1, count, None).await {
        Ok(v) => v,
        Err(e) => {
            red_print!("error: {}", e);
            return;
        }
    };

    if peer.is_empty() {
        green_println!("Found no peers !!!");
    } else {
        green_println!("Found {} peers, listed below: ", peer.len());
        for (i, item) in peer.iter().enumerate() {
            green_println!("peer [{}]: {}", i, item);
        }
    }
}
