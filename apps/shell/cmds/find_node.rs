use boson::dht::Node;
use clap::{arg, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("findnode")
        .visible_alias("find_node")
        .about("Look up a node by id")
        .arg(arg!(<ID> "Target node id (base58)"))
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let id_str = matches.get_one::<String>("ID").unwrap();
    let Ok(target) = id_str.parse() else {
        println!("\x1b[31mInvalid id '{id_str}'\x1b[0m");
        return;
    };
    println!("Attempting to find node with id: {target} ...");

    match node.find_node(&target, None).await {
        Ok(Some(found)) => println!("\x1b[32mFound node: {}\x1b[0m", found),
        Ok(_) => println!("\x1b[32mFound no nodes !!!!\x1b[0m"),
        Err(e) => println!("\x1b[31merror:{}\x1b[0m", e),
    }
}
