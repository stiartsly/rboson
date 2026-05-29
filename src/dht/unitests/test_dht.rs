use std::sync::{Arc, Mutex};

use crate::{
    Network,
    CryptoIdentity,
};
use crate::dht::{
    dht::DHT,
    token_manager::TokenManager,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ipv4() {
        let identity = Arc::new(CryptoIdentity::new());
        let tokenman = Arc::new(TokenManager::new());
        let storage: Arc<Mutex<Box<dyn DataStorage>>> = Arc::new(Mutex::new(Box::new(SqliteStorage::new())));

        let dht = DHT::new_shared(
            identity.clone(),
            Network::IPv4,
            "127.0.0.1".to_string(),
            0,
            None,
            Vec::new(),
            storage,
            tokenman,
        ).unwrap();

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
