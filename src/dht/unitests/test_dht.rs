use std::sync::Arc;
use crate::{
    Network,
    CryptoIdentity,
};
use crate::dht::{
    unitests::test_utils::make_test_dht,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dht4() {
        let identity = Arc::new(CryptoIdentity::new());
        let dht = make_test_dht(identity.clone(), Network::IPv4, "127.0.0.1");

        let mut locked_dht = dht.lock().unwrap();
        locked_dht.start().await.expect("Failed to deploy DHT");

        assert_eq!(locked_dht.network().is_ipv4(), true);
        assert_eq!(locked_dht.id(), identity.id());
        assert_eq!(locked_dht.addr().ip().to_string(), "127.0.0.1");
        assert_eq!(locked_dht.rt().size(), 1);

        locked_dht.stop().await;
        locked_dht.start().await.expect("Failed to restart DHT");

        assert_eq!(locked_dht.network().is_ipv4(), true);
        assert_eq!(locked_dht.id(), identity.id());
        assert_eq!(locked_dht.addr().ip().to_string(), "127.0.0.1");
        assert_eq!(locked_dht.rt().size(), 1);

        locked_dht.stop().await;
    }
}
