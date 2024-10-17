use std::thread;
use tokio::time::Duration;
use clap::Parser;

use boson::{
    Id,
    Node,
    configuration as cfg
};

#[derive(Parser, Debug)]
#[command(about = "Boson Shell", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    let opts = Options::parse();
    let cfg = cfg::Builder::new()
        .load(opts.config.as_ref().map_or("default.conf", |v|&v))
        .map_err(|e| eprintln!("{e}"))
        .unwrap()
        .build()
        .unwrap();

    #[cfg(feature = "inspect")] {
        cfg.dump();
    }

    let node = Node::new(&cfg).unwrap();
    let _ = node.start();

    thread::sleep(Duration::from_secs(1));

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
    match node.find_peer(&peerid, None, None).await {
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
    node.stop();
}
