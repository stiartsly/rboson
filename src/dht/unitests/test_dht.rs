use std::sync::{Arc, Mutex};

use crate::{
    Network,
    crypto_identity::CryptoIdentity,
    signature,
};

use crate::dht::{
    dht::DHT,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
    token_manager::TokenManager,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_and_stop() {
        let keypair = signature::KeyPair::random();
        let identity = Arc::new(Mutex::new(CryptoIdentity::from_keypair(keypair)));
        let expected_id = identity.lock().unwrap().id().clone();
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
        dht.lock().unwrap().start().await.expect("Failed to deploy DHT");

        {
            let locked_dht = dht.lock().unwrap();
            assert_eq!(locked_dht.network().is_ipv4(), true);
            assert_eq!(locked_dht.id(), &expected_id);
            assert_eq!(locked_dht.addr().ip().to_string(), "127.0.0.1");
            assert_eq!(locked_dht.rt().lock().unwrap().size(), 1);
        }

        dht.lock().unwrap().stop().await;

        dht.lock().unwrap().start().await.expect("Failed to restart DHT");

        {
            let locked_dht = dht.lock().unwrap();
            assert_eq!(locked_dht.network().is_ipv4(), true);
            assert_eq!(locked_dht.id(), &expected_id);
            assert_eq!(locked_dht.addr().ip().to_string(), "127.0.0.1");
            assert_eq!(locked_dht.rt().lock().unwrap().size(), 1);
        }

        dht.lock().unwrap().stop().await;
    }
}
