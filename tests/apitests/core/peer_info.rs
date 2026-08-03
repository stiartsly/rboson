use std::sync::{Arc, Mutex};
use boson::{
    Id,
    PeerInfo,
    PeerBuilder,
    CryptoIdentity,
    Identity,
    signature,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test] //case1
    fn test_new() {
        let endpoint = "http://localhost:8080";
        let rc = PeerBuilder::new(endpoint)
            .build();
        assert!(rc.is_ok());

        let peer = rc.unwrap();
        assert_eq!(peer.has_private_key(), true);
        assert_eq!(peer.private_key().is_some(), true);
        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.sequence_number(), 0);
        assert_eq!(peer.fingerprint(), 0);
        assert_eq!(peer.nodeid(), None);
        assert_eq!(peer.is_authenticated(), false);
        assert_eq!(peer.has_extra(), false);
        assert_eq!(peer.is_valid(), true);
    }

    #[test] // case2
    fn test_with_keypair() {
        let endpoint = "http://localhost:8080";
        let keypair = signature::KeyPair::random();
        let rc = PeerBuilder::new(endpoint)
            .with_key(keypair.clone())
            .with_sequence_number(100)
            .with_fingerprint(5)
            .build();
        let peer = rc.unwrap();

        assert_eq!(peer.id(), &Id::from(keypair.public_key()));
        assert_eq!(peer.has_private_key(), true);
        assert_eq!(peer.private_key().is_some(), true);
        assert_eq!(peer.private_key(), Some(keypair.private_key()));
        assert_eq!(peer.nodeid(), None);
        assert_eq!(peer.node_signature(), None);
        assert_eq!(peer.is_authenticated(), false);
        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.sequence_number(), 100);
        assert_eq!(peer.fingerprint(), 5);
        assert_eq!(peer.has_extra(), false);
        assert_eq!(peer.extra_data(), None);
        assert_eq!(peer.signature().len(), 64);
        assert_eq!(peer.is_valid(), true);
    }

    #[test] // case3
    fn test_with_nodeid() {
        let endpoint = "http://localhost:8080";
        let keypair = signature::KeyPair::random();
        let identity = CryptoIdentity::from(keypair);
        let node = Arc::new(Mutex::new(identity));
        let rc = PeerBuilder::new(endpoint)
            .with_node(node.clone())
            .with_sequence_number(101)
            .with_fingerprint(100)
            .build();
        let peer = rc.unwrap();

        assert_eq!(peer.has_private_key(), true);
        assert_eq!(peer.private_key().is_some(), true);
        assert_eq!(peer.nodeid(), Some(node.lock().unwrap().id()));
        assert_eq!(peer.node_signature().is_some(), true);
        assert_eq!(peer.node_signature().unwrap().len(), 64);
        assert_eq!(peer.is_authenticated(), true);
        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.sequence_number(), 101);
        assert_eq!(peer.fingerprint(), 100);
        assert_eq!(peer.has_extra(), false);
        assert_eq!(peer.extra_data(), None);
        assert_eq!(peer.signature().len(), 64);
        assert_eq!(peer.is_valid(), true);
    }

    #[test] // case4
    fn test_full() {
        let endpoint = "http://localhost:8080";
        let node_kp = signature::KeyPair::random();
        let node_identity = CryptoIdentity::from(node_kp);
        let node = Arc::new(Mutex::new(node_identity));
        let peer_kp = signature::KeyPair::random();
        let rc = PeerBuilder::new(endpoint)
            .with_key(peer_kp.clone())
            .with_node(node.clone())
            .with_sequence_number(101)
            .with_fingerprint(100)
            .build();
        let peer = rc.unwrap();

        assert_eq!(peer.id(), &Id::from(peer_kp.public_key()));
        assert_eq!(peer.has_private_key(), true);
        assert_eq!(peer.private_key().is_some(), true);
        assert_eq!(peer.private_key(), Some(peer_kp.private_key()));
        assert_eq!(peer.nodeid(), Some(node.lock().unwrap().id()));
        assert_eq!(peer.node_signature().is_some(), true);
        assert_eq!(peer.node_signature().unwrap().len(), 64);
        assert_eq!(peer.is_authenticated(), true);
        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.sequence_number(), 101);
        assert_eq!(peer.fingerprint(), 100);
        assert_eq!(peer.has_extra(), false);
        assert_eq!(peer.extra_data(), None);
        assert_eq!(peer.signature().len(), 64);
        assert_eq!(peer.is_valid(), true);
    }

    #[test] // case5
    fn test_equal() {
        let endpoint = "http://localhost:8080";
        let kp = signature::KeyPair::random();
        let mut nonce = vec![0u8; PeerInfo::NONCE_BYTES];
        rand::fill(&mut nonce);
        let rc1 = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .build();
        let peer1 = rc1.unwrap();

        let rc2 = PeerBuilder::new(endpoint)
            .with_key(kp)
            .build();
        let peer2 = rc2.unwrap();

        assert_eq!(peer1.id(), peer2.id());
        assert_eq!(peer1.private_key(), peer2.private_key());
        assert_eq!(peer1.nodeid(), peer2.nodeid());
        assert_eq!(peer1.node_signature(), peer2.node_signature());
        assert_eq!(peer1.is_authenticated(), false);
        assert_eq!(peer2.is_authenticated(), false);
        assert_eq!(peer1.sequence_number(), peer2.sequence_number());
        assert_eq!(peer1.endpoint(), peer2.endpoint());
        assert_eq!(peer1.fingerprint(), peer2.fingerprint());
        assert_eq!(peer1, peer2);
    }

    #[test] // case6
    fn test_whole_equal() {
        let endpoint = "http://localhost:8080";
        let node_kp = signature::KeyPair::random();
        let node_identity = CryptoIdentity::from(node_kp);
        let node = Arc::new(Mutex::new(node_identity));
        let peer_kp = signature::KeyPair::random();
        let mut nonce = vec![0u8; PeerInfo::NONCE_BYTES];
        rand::fill(&mut nonce);
        let rc = PeerBuilder::new(endpoint)
            .with_key(peer_kp.clone())
            .with_node(node.clone())
            .with_sequence_number(101)
            .with_fingerprint(100)
            .build();
        let peer1 = rc.unwrap();

        let rc = PeerBuilder::new(endpoint)
            .with_key(peer_kp.clone())
            .with_node(node.clone())
            .with_sequence_number(101)
            .with_fingerprint(100)
            .build();
        let peer2 = rc.unwrap();

        assert_eq!(peer1, peer2);
    }

    #[test] // case9
    fn test_equal_partial() {
        let endpoint = "http://localhost:8080";
        let node_kp = signature::KeyPair::random();
        let node_identity = CryptoIdentity::from(node_kp);
        let node = Arc::new(Mutex::new(node_identity));
        let peer_kp = signature::KeyPair::random();

        let rc = PeerBuilder::new(endpoint).build();
        let peer1 = rc.unwrap();

        let rc = PeerBuilder::new(endpoint)
            .with_key(peer_kp.clone())
            .with_node(node.clone())
            .with_sequence_number(101)
            .with_fingerprint(100)
            .build();
        let peer2 = rc.unwrap();

        assert_ne!(peer1, peer2);
    }

    #[test]
    fn test_serde_cbor_simple() {
        let endpoint = "http://localhost:8080";
        let keypair = signature::KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(keypair.clone())
            .with_sequence_number(10)
            .with_fingerprint(20)
            .build()
            .expect("Failed to build peer info");

        let serialized = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
        let deserialized: PeerInfo = serde_cbor::from_slice(&serialized)
            .expect("Failed to deserialize PeerInfo");

        assert_eq!(peer.id(), deserialized.id());
        assert_eq!(peer.endpoint(), deserialized.endpoint());
        assert_eq!(peer.signature(), deserialized.signature());
        assert_eq!(deserialized.sequence_number(), 10);
        assert_eq!(deserialized.fingerprint(), 20);
        assert_eq!(deserialized.nodeid(), None);
        assert_eq!(deserialized.node_signature(), None);
        assert_eq!(deserialized.extra_data(), None);
        assert_eq!(deserialized.has_private_key(), false);
        assert!(deserialized.is_valid());
    }

    #[test]
    fn test_serde_cbor_with_node() {
        let endpoint = "http://localhost:8080";
        let node_kp = signature::KeyPair::random();
        let node_identity = CryptoIdentity::from(node_kp);
        let node = Arc::new(Mutex::new(node_identity));
        let peer_kp = signature::KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(peer_kp.clone())
            .with_node(node.clone())
            .with_sequence_number(101)
            .with_fingerprint(100)
            .build()
            .expect("Failed to build peer info");

        let serialized = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
        let deserialized: PeerInfo = serde_cbor::from_slice(&serialized)
            .expect("Failed to deserialize PeerInfo");

        assert_eq!(peer.id(), deserialized.id());
        assert_eq!(peer.nodeid(), deserialized.nodeid());
        assert_eq!(peer.node_signature(), deserialized.node_signature());
        assert_eq!(peer.is_authenticated(), true);
        assert_eq!(peer.sequence_number(), deserialized.sequence_number());
        assert_eq!(peer.endpoint(), deserialized.endpoint());
        assert_eq!(peer.fingerprint(), deserialized.fingerprint());
        assert_eq!(deserialized.has_private_key(), false);
        assert!(deserialized.is_valid());
    }

    #[test]
    fn test_serde_json() {
        let endpoint = "http://localhost:8080";
        let keypair = signature::KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(keypair.clone())
            .with_sequence_number(5)
            .with_fingerprint(7)
            .build()
            .expect("Failed to build peer info");

        let json = serde_json::to_string(&peer).expect("Failed to serialize PeerInfo to JSON");
        let deserialized: PeerInfo = serde_json::from_str(&json)
            .expect("Failed to deserialize PeerInfo from JSON");

        assert_eq!(peer.id(), deserialized.id());
        assert_eq!(peer.endpoint(), deserialized.endpoint());
        assert_eq!(peer.signature(), deserialized.signature());
        assert_eq!(deserialized.sequence_number(), 5);
        assert_eq!(deserialized.fingerprint(), 7);
        assert!(deserialized.is_valid());
    }

    #[test]
    fn test_serde_rejects_tampered_signature() {
        let endpoint = "http://localhost:8080";
        let keypair = signature::KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(keypair.clone())
            .build()
            .expect("Failed to build peer info");

        let mut serialized = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
        // Flip the last serialized byte; this corrupts the CBOR structure or signature.
        let last = serialized.len() - 1;
        serialized[last] = serialized[last].wrapping_add(1);

        let result: Result<PeerInfo, _> = serde_cbor::from_slice(&serialized);
        assert!(result.is_err(), "Deserializing a tampered PeerInfo should fail");
    }
}
