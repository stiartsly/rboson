use std::thread;
use tokio::{
    task::LocalSet,
    time::Duration,
};
use clap::Parser;

use boson::{
    Id,
    dht::{
        Node,
        NodeConfig,
        NodeConfiguration
    },
};

#[derive(Parser, Debug)]
#[command(about = "Boson Shell", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    #[arg(short='S', long)]
    simulate: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let local = LocalSet::new();
    local.run_until(async {
        let opts = Options::parse();
        let config = NodeConfiguration::load(
            opts.config.as_deref().unwrap_or("node.yaml")
        ).unwrap();

        #[cfg(feature = "inspect")] {
            config.dump();
        }

        let bootstrap_nodes = config.bootstrap_nodes().to_vec();

        let node = Node::new(Box::new(config)).unwrap();
        let _ = node.start().await;

        if opts.simulate {
            let _ = node.bootstrap_one(&bootstrap_nodes[0]).await;
        }

        thread::sleep(Duration::from_secs(10*60));

        let target: Id = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ".try_into().unwrap();
        println!("Attemp finding node with id: {} ...", target);
        match node.find_node(&target, None).await {
            Ok(val) => match val.v4() {
                Some(node) => println!("Found node: {}", node.to_string()),
                None => println!("Not found !!!!"),
            },
            Err(e) => println!("error: {}", e),
        }

        thread::sleep(Duration::from_secs(2));
        let peerid: Id = "5vVM1nrCwFh3QqAgbvF3bRgYQL5a2vpFjngwxkiS8Ja6".try_into().unwrap();
        println!("Attemp finding peers with id: {} ...", peerid);
        match node.find_peer(&peerid, -1, 8, None).await {
            Ok(val) => {
                if val.is_empty() {
                    println!("Found no peers, try to lookup it later !!!")
                } else {
                    println!("Found {} peers, listed below: ", val.len());
                    let mut i = 0;
                    for item in val.iter() {
                        println!("peer [{}]: {}", i, item);
                        i+=1;
                    }
                }
            },
            Err(e) => println!("error: {}", e),
        }

        thread::sleep(Duration::from_secs(10));
        let _ = node.stop().await;
    }).await;
}
