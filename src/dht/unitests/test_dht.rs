use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use crate::{
    Network,
    Identity,
    CryptoIdentity,
};
use crate::dht::{
    connection_status_listener::ConnectionStatusListener,
    dht::DHT,
    dht_verticle::VerticleOptions,
    promise::Promise,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
    timer_client::{LocalTimerClient, LocalTimerCmd},
    token_manager::TokenManager,
};

struct NoopConnectionStatusListener;
impl ConnectionStatusListener for NoopConnectionStatusListener {}

pub(super) fn make_dht(
    identity: Arc<CryptoIdentity>,
    network: Network,
    host: &str,
) -> (Rc<RefCell<DHT>>, mpsc::UnboundedReceiver<LocalTimerCmd>) {
    let tokenman = Arc::new(TokenManager::new());
    let storage: Arc<Mutex<dyn DataStorage>> = Arc::new(Mutex::new(SqliteStorage::new()));
    let listener: Arc<dyn ConnectionStatusListener> = Arc::new(NoopConnectionStatusListener);
    let (tx, rx) = mpsc::unbounded_channel::<LocalTimerCmd>();
    let timer_client = Rc::new(LocalTimerClient::new(tx));
    let data_dir = PathBuf::from(".");

    let options = VerticleOptions::default()
        .with_identity(identity)
        .with_storage(storage)
        .with_tokenman(tokenman)
        .with_listener(listener)
        .with_datadir(data_dir);

    let dht = DHT::new(options, network, host.to_string(), 0, None, timer_client)
        .expect("test DHT should build");

    let dht = Rc::new(RefCell::new(dht));
    dht.borrow_mut().weak = Rc::downgrade(&dht);
    (dht, rx)
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
                let _ = dht.borrow_mut().start0().await;
                let _ = dht.borrow_mut().start(promise).await;
                future.await.expect("start promise should resolve");

                assert_eq!(dht.borrow().network().is_ipv4(), true);
                assert_eq!(dht.borrow().id(), identity.id());
                assert_eq!(dht.borrow().ni().socket_addr().ip().to_string(), "127.0.0.1");
                assert_eq!(dht.borrow().rt().borrow().size(), 1);

                dht.borrow_mut().stop().await;

                let (promise, future) = Promise::pair();
                let _ = dht.borrow_mut().start0().await;
                let _ = dht.borrow_mut().start(promise).await;
                future.await.expect("start promise should resolve");

                assert_eq!(dht.borrow().network().is_ipv4(), true);
                assert_eq!(dht.borrow().id(), identity.id());
                assert_eq!(dht.borrow().ni().socket_addr().ip().to_string(), "127.0.0.1");
                assert_eq!(dht.borrow().rt().borrow().size(), 1);

                println!("Stopping DHT >>> line:{}", line!());
                dht.borrow_mut().stop().await;
            }));

            println!("DHT verticle thread exiting >>> line:{}", line!());
        });
        handle.join().unwrap();
    }
}
