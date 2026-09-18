use boson::dht::Node;
use clap::{arg, ArgMatches, Command};

use super::parse_id;

pub(crate) fn command() -> Command {
    Command::new("findvalue")
        .visible_alias("find_value")
        .about("Look up a value by id")
        .arg(arg!(<ID> "Target value id (base58)"))
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let Some(valueid) = parse_id(matches.get_one::<String>("ID").unwrap()) else {
        return;
    };
    println!("Attempting to find value with id: {valueid} ...");
    match node.find_value(&valueid, -1, None).await {
        Ok(Some(val)) => println!("\x1b[32mFound value: {}\x1b[0m", val),
        Ok(_) => println!("\x1b[32mFound no values !!!!\x1b[0m"),
        Err(e) => println!("\x1b[31merror: {}\x1b[0m", e),
    }
}
