use crate::dht::{
    connection_status_listener::ConnectionStatusListener,
    dht::DHT,
    dht_verticle::VerticleOptions,
    storage::{data_storage::DataStorage, sqlite_storage::SqliteStorage},
    token_manager::TokenManager,
};
use crate::{CryptoIdentity, Identity, LocalBoxTimerClient, LocalBoxTimerCmd, Network, Promise};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

struct NoopConnectionStatusListener;
impl ConnectionStatusListener for NoopConnectionStatusListener {}

pub(super) fn make_dht(
    identity: Arc<CryptoIdentity>,
    network: Network,
    host: &str,
) -> (Rc<DHT>, mpsc::UnboundedReceiver<LocalBoxTimerCmd>) {
    let tokenman = Arc::new(TokenManager::new());
    let storage: Arc<Mutex<dyn DataStorage>> = Arc::new(Mutex::new(SqliteStorage::new()));
    let listener: Arc<dyn ConnectionStatusListener> = Arc::new(NoopConnectionStatusListener);
    let (tx, rx) = mpsc::unbounded_channel::<LocalBoxTimerCmd>();
    let timer_client = Rc::new(LocalBoxTimerClient::new(tx));

    let options = VerticleOptions {
        identity: identity.clone(),
        storage: storage.clone(),
        token_man: tokenman.clone(),
        listener: listener.clone(),
        bootstrap_nodes: Vec::new(),
    };

    let dht = DHT::new(options, network, host.to_string(), 0, None, timer_client);
    (dht, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }))
        .await;
        let val = increment.lock().unwrap().load(Ordering::SeqCst);
        assert_eq!(val, 10);
    }

    #[tokio::test]
    async fn test_dht4() {
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
                .expect("dht verticle runtime should build");

            let local = tokio::task::LocalSet::new();
            rt.block_on(local.run_until(async move {
                let identity = Arc::new(CryptoIdentity::new());
                let (dht, _timer_rx) = make_dht(identity.clone(), Network::IPv4, "127.0.0.1");

                let (promise, future) = Promise::pair();
                let _ = dht.start0().await;
                let _ = dht.start(promise).await;
                future.await.expect("start promise should resolve");

                assert_eq!(dht.network().is_ipv4(), true);
                assert_eq!(dht.id(), identity.id());
                assert_eq!(dht.ni().address().ip().to_string(), "127.0.0.1");
                assert_eq!(dht.rt().size(), 1);

                dht.stop().await;

                let (promise, future) = Promise::pair();
                let _ = dht.start0().await;
                let _ = dht.start(promise).await;
                future.await.expect("start promise should resolve");

                assert_eq!(dht.network().is_ipv4(), true);
                assert_eq!(dht.id(), identity.id());
                assert_eq!(dht.ni().address().ip().to_string(), "127.0.0.1");
                assert_eq!(dht.rt().size(), 1);

                println!("Stopping DHT >>> line:{}", line!());
                dht.stop().await;
            }));

            println!("DHT verticle thread exiting >>> line:{}", line!());
        });
        handle.join().unwrap();
    }
}
