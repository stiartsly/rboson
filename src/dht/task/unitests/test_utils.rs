use crate::dht::{
    connection_status_listener::ConnectionStatusListener,
    dht::DHT,
    dht_verticle::VerticleOptions,
    storage::{data_storage::DataStorage, sqlite_storage::SqliteStorage},
    token_manager::TokenManager,
};
use crate::{CryptoIdentity, LocalBoxTimerClient, LocalBoxTimerCmd, Network};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

struct NoopConnectionStatusListener;
impl ConnectionStatusListener for NoopConnectionStatusListener {}

/// Create a minimal in-process DHT for unit tests. The node binds to
/// `host:0` (OS-assigned port), uses an in-memory SQLite store, and
/// has no bootstrap nodes.
pub(super) fn make_test_dht(network: Network, host: &str) -> Rc<DHT> {
    let identity = Arc::new(CryptoIdentity::new());
    let storage: Arc<Mutex<dyn DataStorage>> = Arc::new(Mutex::new(SqliteStorage::new()));
    let token_man = Arc::new(TokenManager::new());
    let listener: Arc<dyn ConnectionStatusListener> = Arc::new(NoopConnectionStatusListener);

    let options = VerticleOptions {
        identity,
        storage,
        token_man,
        listener,
        bootstrap_nodes: Vec::new(),
    };
    let (tx, _rx) = mpsc::unbounded_channel::<LocalBoxTimerCmd>();
    let timer_client = Rc::new(LocalBoxTimerClient::new(tx));

    DHT::new(options, network, host.to_string(), 0, None, timer_client)
}
