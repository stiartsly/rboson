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
        assert_eq!(inner_identity.id(), &expected_id);
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
    fn test_context_different_keys() {
        let cached = CachedIdentity::new(CryptoIdentity::new());
        let peer_a = CryptoIdentity::new().id().clone();
        let peer_b = CryptoIdentity::new().id().clone();

        let ctx_a = cached.context(&peer_a);
        let ctx_b = cached.context(&peer_b);

        assert!(!Arc::ptr_eq(&ctx_a, &ctx_b));
        assert_eq!(ctx_a.lock().unwrap().id(), &peer_a);
        assert_eq!(ctx_b.lock().unwrap().id(), &peer_b);
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

    #[test]
    fn test_sign_and_verify() {
        let cached = CachedIdentity::new(CryptoIdentity::new());
        let data = b"Hello, Boson!";

        let sig = cached.sign_into(data).unwrap();
        let ok = cached.verify(data, &sig).unwrap();
        assert!(ok);
    }

    #[test]
    fn test_sign_tampered_data() {
        let cached = CachedIdentity::new(CryptoIdentity::new());
        let data = b"Hello, Boson!";
        let tampered = b"Hello, World!";

        let sig = cached.sign_into(data).unwrap();
        let ok = cached.verify(tampered, &sig).unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_sign_wrong_identity() {
        let alice = CachedIdentity::new(CryptoIdentity::new());
        let bob   = CachedIdentity::new(CryptoIdentity::new());
        let data  = b"Hello, Boson!";

        let sig = alice.sign_into(data).unwrap();
        // Bob's verify checks against Bob's own public key — should reject Alice's signature.
        let ok = bob.verify(data, &sig).unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let alice = CachedIdentity::new(CryptoIdentity::new());
        let bob   = CachedIdentity::new(CryptoIdentity::new());
        let plain = b"Hello, Boson!";

        let cipher = alice.encrypt_into(bob.id(), plain).unwrap();
        let decrypted = bob.decrypt_into(alice.id(), &cipher).unwrap();

        assert_eq!(plain.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_multiple_messages() {
        let alice = CachedIdentity::new(CryptoIdentity::new());
        let bob   = CachedIdentity::new(CryptoIdentity::new());

        for i in 0u8..5 {
            let plain = vec![i; 32];
            let cipher = alice.encrypt_into(bob.id(), &plain).unwrap();
            let decrypted = bob.decrypt_into(alice.id(), &cipher).unwrap();
            assert_eq!(plain, decrypted);
        }
    }
}
