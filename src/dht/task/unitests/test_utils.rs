use std::{
    rc::Rc,
    cell::RefCell,
    sync::{Arc, Mutex},
    path::PathBuf,
};
use tokio::sync::mpsc;
use crate::CryptoIdentity;
use crate::Network;
use crate::dht::{
    dht::DHT,
    dht_verticle::VerticleOptions,
    timer_client::{LocalTimerClient, LocalTimerCmd},
    token_manager::TokenManager,
    connection_status_listener::ConnectionStatusListener,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
};

struct NoopConnectionStatusListener;
impl ConnectionStatusListener for NoopConnectionStatusListener {}

/// Create a minimal in-process DHT for unit tests. The node binds to
/// `host:0` (OS-assigned port), uses an in-memory SQLite store, and
/// has no bootstrap nodes.
pub(super) fn make_test_dht(network: Network, host: &str) -> Rc<RefCell<DHT>> {
    let identity  = Arc::new(CryptoIdentity::new());
    let storage: Arc<Mutex<dyn DataStorage>> = Arc::new(Mutex::new(SqliteStorage::new()));
    let token_man = Arc::new(TokenManager::new());
    let listener: Arc<dyn ConnectionStatusListener> = Arc::new(NoopConnectionStatusListener);

    let options = VerticleOptions::default()
        .with_identity(identity)
        .with_storage(storage)
        .with_tokenman(token_man)
        .with_listener(listener)
        .with_datadir(PathBuf::from("."));

    let (tx, _rx) = mpsc::unbounded_channel::<LocalTimerCmd>();
    let timer_client = Rc::new(LocalTimerClient::new(tx));

    let dht = DHT::new(options, network, host.to_string(), 0, None, timer_client)
        .expect("test DHT should build");

    let dht = Rc::new(RefCell::new(dht));
    dht.borrow_mut().weak = Rc::downgrade(&dht);
    dht
}
