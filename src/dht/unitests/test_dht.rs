use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{sync::mpsc, runtime};
use crate::{
    Network,
    CryptoIdentity,
    NodeInfo,
};
use crate::dht::{
    dht::{Builder, DHT},
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
    timer_client::TimerClient,
    timer_queue::{Command, TimerQueue},
    token_manager::TokenManager,
};

pub(super) fn make_dht(
    identity: Arc<CryptoIdentity>,
    network: Network,
    host: &str,
) -> Arc<Mutex<DHT>> {
    let tokenman = Arc::new(TokenManager::new());
    let storage: Arc<Mutex<Box<dyn DataStorage>>> = Arc::new(Mutex::new(Box::new(SqliteStorage::new())));
    let (tx, rx) = mpsc::channel::<Command>(64);
    let timer_client = Arc::new(TimerClient::new(tx));
    let bootstrap_nodes: Vec<NodeInfo> = Vec::new();
    let data_dir = PathBuf::from(".");

    std::thread::spawn(move || {
        let runtime = runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("timer runtime should build");

        runtime.block_on(async move {
            TimerQueue::new(rx).run().await;
        });
    });

    let mut builder = Builder::default();
    builder
        .with_identity(identity)
        .with_storage(storage)
        .with_tokenman(tokenman)
        .with_timer_client(timer_client)
        .with_bootstrap_nodes(&bootstrap_nodes)
        .with_datadir(data_dir.as_path());

    let dht = match network {
        Network::IPv4 => builder.build_dht4(host, 0),
        Network::IPv6 => builder.build_dht6(host, 0),
    }
    .expect("test DHT should build");

    let dht = Arc::new(Mutex::new(dht));
    dht.lock().unwrap().weak = Arc::downgrade(&dht);
    dht
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dht4() {
        let identity = Arc::new(CryptoIdentity::new());
        let dht = make_dht(identity.clone(), Network::IPv4, "127.0.0.1");

        let mut locked_dht = dht.lock().unwrap();
        locked_dht.start().await.expect("Failed to deploy DHT");

        assert_eq!(locked_dht.network().is_ipv4(), true);
        assert_eq!(locked_dht.id(), identity.id());
        assert_eq!(locked_dht.addr().ip().to_string(), "127.0.0.1");
        assert_eq!(locked_dht.rt().lock().unwrap().size(), 1);

        locked_dht.stop().await;
        locked_dht.start().await.expect("Failed to restart DHT");

        assert_eq!(locked_dht.network().is_ipv4(), true);
        assert_eq!(locked_dht.id(), identity.id());
        assert_eq!(locked_dht.addr().ip().to_string(), "127.0.0.1");
        assert_eq!(locked_dht.rt().lock().unwrap().size(), 1);

        locked_dht.stop().await;
    }
}
