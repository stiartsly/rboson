use std::sync::{Arc, Mutex};

use crate::{
    Id,
    Network,
    crypto_identity::CryptoIdentity,
};

use crate::dht::{
    dht::DHT,
    task::{
        lookup_task::LookupTask,
        peer_lookup::PeerLookupTask,
        task::Task
    },
    token_manager::TokenManager,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
};

fn make_dht() -> Arc<Mutex<DHT>> {
    let identity = Arc::new(Mutex::new(CryptoIdentity::new()));
    let storage: Arc<Mutex<Box<dyn DataStorage>>> = Arc::new(Mutex::new(Box::new(SqliteStorage::new())));
    let tokenman = Arc::new(Mutex::new(TokenManager::new()));

    let dht = Arc::new(Mutex::new(
        DHT::new(
            identity,
            Network::IPv4,
            "127.0.0.1".to_string(),
            0,
            None,
            Vec::new(),
            storage,
            tokenman,
        ).unwrap()
    ));
    dht.lock().unwrap().set_cloned(dht.clone());
    dht
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_task() {
        let target = Id::random();
        let task = PeerLookupTask::new(make_dht(), target.clone(), 7, 3, true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);
        assert_eq!(task.result().is_empty(), true);
    }
}
