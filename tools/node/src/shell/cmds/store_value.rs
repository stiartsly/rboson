use clap::{arg, ArgMatches, Command};
use boson::{dht::Node, ImmutableBuilder};

pub(crate) fn command() -> Command {
    Command::new("announcevalue")
        .about("Announce an immutable value to the Boson network")
        .arg(arg!(<VALUE> "Value data (string) to announce"))
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let value = matches.get_one::<String>("VALUE").unwrap();
    announce(node, value).await;
}

pub(crate) async fn announce(node: &Node, data: &str) {
    let value = match ImmutableBuilder::new(data.as_bytes()).build() {
        Ok(v) => v,
        Err(e) => {
            red_print!("Error building value: {e}");
            return;
        }
    };

    let valueid = value.id();
    println!(
        "Announcing value {} ({} bytes) ...",
        valueid,
        value.data().len()
    );

    match node.store_value(&value, -1, false).await {
        Ok(_) => green_println!("Value {} announced successfully.", valueid),
        Err(e) => red_print!("Failed to announce value {}: {}", valueid, e),
    }
}
