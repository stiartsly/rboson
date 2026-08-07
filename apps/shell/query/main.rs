use std::{
    thread,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    task::LocalSet,
};
use clap::Parser;
use tokio::sync::Notify;

use boson::{
    Id,
    NodeConfig,
    cfg::configuration,
    dht::{
        Node,
        ConnectionStatus,
    },
    network::Network,
    connection_status_listener::ConnectionStatusListener,
};

#[derive(Default)]
struct DefaultConnectionStatusListener {
    ready: Option<Arc<Notify>>,
    status: Option<Arc<Mutex<ConnectionStatus>>>,
}

impl ConnectionStatusListener for DefaultConnectionStatusListener {
    fn status_changed(&self,
        network: Network,
        new_status: ConnectionStatus,
        old_status: ConnectionStatus,
    ) {
        println!("\x1b[34mConnection status changed for network {}: {}->{}\x1b[0m", network, old_status, new_status);
        if let Some(status) = self.status.as_ref() {
            *status.lock().unwrap() = new_status;
        }
    }
    fn connecting(&self, network: Network) {
        println!("\x1b[34mConnecting to network {}...\x1b[0m", network);
    }
    fn connected(&self, network: Network) {
        println!("\x1b[34mConnected to network {}.\x1b[0m", network);
        if let Some(ready) = self.ready.as_ref() {
            ready.notify_one();
        }
    }
    fn disconnected(&self, network: Network) {
        println!("\x1b[34mDisconnected from network {}.\x1b[0m", network);
    }
}

#[derive(Parser, Debug)]
#[command(about = "Boson Shell", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    #[arg(short='Q', long)]
    query: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let local = LocalSet::new();
    local.run_until(async {
        let opts = Options::parse();
        let config = match configuration::Builder::new()
                .load(opts.config.as_deref().unwrap_or("node.yaml"))
                .and_then(|b| b.build())
        {
            Ok(v) => v,
            Err(e) => {
                println!("Loading configuration failed: {e}");
                return;
            }
        };

        #[cfg(feature = "inspect")] {
            config.dump();
        }

        let ready = Arc::new(Notify::new());
        let status = Arc::new(Mutex::new(ConnectionStatus::Disconnected));
        let listener = DefaultConnectionStatusListener {
            ready: Some(ready.clone()),
            status: Some(status.clone()),
        };

        let bootstrap_nodes = config.bootstrap_nodes().to_vec();

        let node = Node::new(Box::new(config)).unwrap();
        node.add_listener(listener);
        let _ = node.start().await;

        if opts.query {
            let _ = node.bootstrap(&bootstrap_nodes).await;
        }

        thread::sleep(Duration::from_secs(5));
        let target: Id = "42Q5zvkSYT6rsbJG7KWv5cyNBYDDmQJmpKvm4nGj2EiN".try_into().unwrap();
        println!("Attempt finding node id: {} ...", target);
        match node.find_node(&target, None).await {
            Ok(Some(v)) => println!("\x1b[34mFound node: {}\x1b[0m", v),
            Ok(_) => println!("\x1b[31mNot found !!!!\x1b[0m"),
            Err(e) => println!("error: {}", e),
        }

        thread::sleep(Duration::from_secs(2));
        let target: Id = "2hAM9jXD96cfj7zJ75oaXX52hnFtTE9sDfhJxQyNHLvL".try_into().unwrap();
        println!("Attempt finding node id: {} ...", target);
        match node.find_node(&target, None).await {
            Ok(Some(v)) => println!("\x1b[34mFound node: {}\x1b[0m", v),
            Ok(_) => println!("\x1b[31mNot found !!!!\x1b[0m"),
            Err(e) => println!("error: {}", e),
        }

        thread::sleep(Duration::from_secs(2));
        let peerid: Id = "88WmYVuFpR1B6L5SJ9k1Xx9jkz225iWQ8qCNoNrrWdvz".try_into().unwrap();
        println!("Attempt finding peer id: {} ...", peerid);
        match node.find_peer(&peerid, -1, 8, None).await {
            Ok(v) => {
                if v.is_empty() {
                    println!("\x1b[31mFound no peers, try to lookup it later !!!\x1b[0m")
                } else {
                    println!("Found {} peer(s), listed below:", v.len());
                    let mut i = 0;
                    for item in v.iter() {
                        println!("\x1b[34mpeer [{}]: {}\x1b[0m", i, item);
                        i+=1;
                    }
                }
            },
            Err(e) => println!("error: {}", e),
        }

        thread::sleep(Duration::from_secs(2));
        let valueid: Id = "6tLSgsSng6m8LCv1VWFxsJT3SW5RWfjMqEAUNS52TsSm".try_into().unwrap();
        println!("Attempt finding value id: {} ...", valueid);
        match node.find_value(&valueid, -1, None).await {
            Ok(Some(v)) => println!("\x1b[34mFound value: {}\x1b[0m", v.to_string()),
            Ok(_) => println!("\x1b[31mNot found !!!!\x1b[0m"),
            Err(e) => println!("error: {}", e),
        }

        let _ = node.stop().await;
    }).await;
}
