use boson::dht::Node;
use clap::Command;

pub(crate) fn command() -> Command {
    Command::new("info")
        .about("Show information and status of the current DHT node")
        .visible_alias("me")
        .visible_alias("status")
}

pub(crate) fn run(node: &Node, connected: bool) {
    println!("+------------------------------------------------------------+");
    println!("|                   Boson DHT Node Info                      |");
    println!("+------------------------------------------------------------+");
    println!("Node ID:        {}", node.id());
    println!("Running:        {}", if node.is_running() { "yes" } else { "no" });
    println!(
        "DHT Connection: {}",
        if connected { "connected" } else { "disconnected" }
    );
    match node.node_info() {
        Ok(node_info) => println!("Node Info:      {node_info}"),
        Err(e) => println!("\x1b[31mUnable to read node information: {e}\x1b[0m"),
    }
    println!("Data Directory: {}", node.options().data_dir());
    println!("Listen Port:    {}", node.options().port());
    if let Some(h4) = node.options().host4() {
        println!("IPv4 Host:      {h4}");
    }
    if let Some(h6) = node.options().host6() {
        println!("IPv6 Host:      {h6}");
    }
    println!(
        "Dev Mode:       {}",
        if node.options().developer_mode() {
            "enabled"
        } else {
            "disabled"
        }
    );
}
