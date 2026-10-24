use boson::dht::Node;
use clap::{arg, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("findvalue")
        .about("Look up a value by id")
        .arg(arg!(<ID> "Target value id (base58)"))
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let id_str = matches.get_one::<String>("ID").unwrap();
    let Ok(valueid) = id_str.parse() else {
        red_print!("Invalid id '{id_str}'");
        return;
    };

    println!("Attempting to find value with id: {valueid} ...");
    match node.find_value(&valueid, -1, None).await {
        Ok(Some(val)) => green_println!("Found value: {}", val),
        Ok(_) => green_println!("Found no values !!!!"),
        Err(e) => red_print!("error: {}", e),
    }
}
