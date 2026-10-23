use boson::dht::Node;
use clap::{arg, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("findnode")
        .about("Look up a node by id")
        .arg(arg!(<ID> "Target node id (base58)"))
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let id_str = matches.get_one::<String>("ID").unwrap();
    let Ok(target) = id_str.parse() else {
        red_print!("Invalid id '{id_str}'");
        return;
    };
    println!("Attempting to find node with id: {target} ...");

    match node.find_node(&target, None).await {
        Ok(Some(found)) => green_println!("Found node: {}", found),
        Ok(_) => green_println!("Found no nodes !!!!"),
        Err(e) => red_print!("error:{}", e),
    }
}
