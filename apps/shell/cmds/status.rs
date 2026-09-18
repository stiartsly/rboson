use boson::dht::Node;
use clap::Command;

pub(crate) fn command() -> Command {
    Command::new("status").about("Show this node's status")
}

pub(crate) fn run(node: &Node, connected: bool) {
    println!("Node id: {}", node.id());
    println!("Node running: {}", node.is_running());
    println!(
        "DHT connection: {}",
        if connected {
            "connected"
        } else {
            "disconnected"
        }
    );
    match node.node_info() {
        Ok(node_info) => println!("Node info: {node_info}"),
        Err(e) => println!("\x1b[31mUnable to read node information: {e}\x1b[0m"),
    }
}
