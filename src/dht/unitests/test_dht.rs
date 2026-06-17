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
    quit_flag: Arc<Mutex<bool>>
) -> Arc<Mutex<DHT>> {
    let tokenman = Arc::new(TokenManager::new());
    let storage: Arc<Mutex<dyn DataStorage>> = Arc::new(Mutex::new(SqliteStorage::new()));
    let (tx, rx) = mpsc::unbounded_channel::<Command>();
    let timer_client = Arc::new(TimerClient::new(tx));
    let bootstrap_nodes: Vec<NodeInfo> = Vec::new();
    let data_dir = PathBuf::from(".");

    std::thread::spawn(move || {
        let runtime = runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("timer runtime should build");

        runtime.block_on(async move {
            TimerQueue::new(rx).run(quit_flag).await;
        });
    });

    let builder = Builder::default()
        .with_identity(identity)
        .with_storage(storage)
        .with_tokenman(tokenman)
        .with_timer_client(timer_client)
        .with_bootstrap(&bootstrap_nodes)
        .with_datadir(data_dir.as_path());

    let dht = builder.build(network, host, 0)
        .expect("test DHT should build");

    let dht = Arc::new(Mutex::new(dht));
    dht.lock().unwrap().weak = Arc::downgrade(&dht);
    dht
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_futures() {
        use std::sync::atomic::{AtomicU8, Ordering};
        use std::time::Duration;

        let increment = Arc::new(Mutex::new(AtomicU8::new(0)));

        futures::future::join_all((0..10).map(|_| {
            let increment = increment.clone();
            async move {
                increment.lock().unwrap().fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })).await;
        let val = increment.lock().unwrap().load(Ordering::SeqCst);
        assert_eq!(val, 10);

        /*
        use futures::stream::{FuturesUnordered, FuturesOrdered, StreamExt};

        let unordered = FuturesUnordered::new();
        for i in 0..10 {
            let increment = increment.clone();
            unordered.push(async move {
                increment.lock().unwrap().fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(100)).await;
            });
        }
        let result = futures::future::join_all(unordered).await;
        let incre = increment.lock().unwrap().load(Ordering::SeqCst);
        assert_eq!(incre, 20);
        */
    }

    #[tokio::test]
    async fn test_dht4() {
        let identity = Arc::new(CryptoIdentity::new());
        let quit_flag = Arc::new(Mutex::new(false));
        let dht = make_dht(identity.clone(), Network::IPv4, "127.0.0.1", quit_flag);

        let mut locked_dht = dht.lock().unwrap();
        locked_dht.start().await.expect("Failed to deploy DHT");

        assert_eq!(locked_dht.network().is_ipv4(), true);
        assert_eq!(locked_dht.id(), identity.id());
        assert_eq!(locked_dht.ni().socket_addr().ip().to_string(), "127.0.0.1");
        assert_eq!(locked_dht.rt().lock().unwrap().size(), 1);

        locked_dht.stop().await;
        locked_dht.start().await.expect("Failed to restart DHT");

        assert_eq!(locked_dht.network().is_ipv4(), true);
        assert_eq!(locked_dht.id(), identity.id());
        assert_eq!(locked_dht.ni().socket_addr().ip().to_string(), "127.0.0.1");
        assert_eq!(locked_dht.rt().lock().unwrap().size(), 1);

        locked_dht.stop().await;
    }
}
