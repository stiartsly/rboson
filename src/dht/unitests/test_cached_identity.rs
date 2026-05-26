use std::sync::Arc;
use crate::{
    Identity,
    core::crypto_identity::CryptoIdentity,
    dht::cached_identity::CachedIdentity,
};


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let identity = CryptoIdentity::new();
        let expected_id = identity.id().clone();
        let cached = CachedIdentity::new(identity);

        assert_eq!(cached.id(), &expected_id);

        let inner_identity = cached.identity();
        assert_eq!(inner_identity.lock().unwrap().id(), &expected_id);
        assert_eq!(Arc::ptr_eq(&inner_identity, &cached.identity()), true);
    }

    #[test]
    fn test_context_with_same_key() {
        let cached = CachedIdentity::new(CryptoIdentity::new());
        let peer_id = CryptoIdentity::new().id().clone();

        let first = cached.context(&peer_id);
        let second = cached.context(&peer_id);

        assert_eq!(Arc::ptr_eq(&first, &second), true);
        assert_eq!(first.lock().unwrap().id(), &peer_id);
    }

    #[test]
    fn test_clear_cache() {
        let cached = CachedIdentity::new(CryptoIdentity::new());
        let peer_id = CryptoIdentity::new().id().clone();

        let first = cached.context(&peer_id);
        cached.clear_cache();
        let second = cached.context(&peer_id);

        assert_eq!(Arc::ptr_eq(&first, &second), false);
        assert_eq!(second.lock().unwrap().id(), &peer_id);
    }
}
