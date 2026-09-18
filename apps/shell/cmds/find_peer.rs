use boson::dht::Node;
use clap::{arg, value_parser, ArgMatches, Command};

use super::parse_id;

pub(crate) fn command() -> Command {
    Command::new("findpeer")
        .visible_alias("find_peer")
        .about("Look up peers announced under an id")
        .arg(arg!(<ID> "Target peer id (base58)"))
        .arg(
            arg!(-c --count <COUNT> "Expected number of peers")
                .default_value("8")
                .value_parser(value_parser!(usize)),
        )
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let Some(peerid) = parse_id(matches.get_one::<String>("ID").unwrap()) else {
        return;
    };
    let count = *matches.get_one::<usize>("count").unwrap();
    println!("Attempting to find peers with id: {peerid} ...");
    match node.find_peer(&peerid, -1, count, None).await {
        Ok(val) => {
            if val.is_empty() {
                println!("\x1b[32mFound no peers !!!\x1b[0m");
            } else {
                println!("\x1b[32mFound {} peers, listed below: \x1b[0m", val.len());
                for (i, item) in val.iter().enumerate() {
                    println!("\x1b[32mpeer [{}]: {}\x1b[0m", i, item);
                }
            }
        }
        Err(e) => println!("\x1b[31merror: {}\x1b[0m", e),
    }
}
