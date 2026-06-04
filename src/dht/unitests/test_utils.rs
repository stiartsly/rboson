use std::sync::{Arc, Mutex};

use tokio::{
    runtime,
    sync::mpsc,
};

use crate::{
    crypto_identity::CryptoIdentity,
    Network,
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

pub(super) fn make_test_dht(
    identity: Arc<CryptoIdentity>,
    network: Network,
    host: &str,
) -> Arc<Mutex<DHT>> {
    let tokenman = Arc::new(TokenManager::new());
    let storage: Arc<Mutex<Box<dyn DataStorage>>> = Arc::new(Mutex::new(Box::new(SqliteStorage::new())));
    let (tx, rx) = mpsc::channel::<Command>(64);
    let timer_client = Arc::new(TimerClient::new(tx));
    let bootstrap_nodes: Vec<NodeInfo> = Vec::new();
    let data_dir = ".";

    std::thread::spawn(move || {
        let runtime = runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("timer runtime should build");

        runtime.block_on(async move {
            TimerQueue::new(rx).run().await;
        });
    });

    let mut builder = Builder::new();
    builder
        .with_identity(identity)
        .with_storage(storage)
        .with_tokenman(tokenman)
        .with_timer_client(timer_client)
        .with_bootstrap_nodes(&bootstrap_nodes)
        .with_datadir(data_dir);

    let dht = match network {
        Network::IPv4 => builder.build_dht4(host, 0),
        Network::IPv6 => builder.build_dht6(host, 0),
    }
    .expect("test DHT should build");

    let dht = Arc::new(Mutex::new(dht));
    dht.lock().unwrap().weak_cloned = Arc::downgrade(&dht);
    dht
}