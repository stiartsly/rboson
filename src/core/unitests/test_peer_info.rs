use std::sync::{Arc, Mutex};
use crate::core::{
    Id,
    signature,
    CryptoIdentity,
    PeerInfo,
    PeerBuilder,
    signature::KeyPair,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_builder() {
        let endpoint = "https://example.com:8080";
        let kp = KeyPair::random();

        let peer = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .build()
            .expect("Failed to build peer info");

        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.id(), &Id::from(kp.public_key()));
        assert_eq!(peer.has_private_key(), true);
        assert!(peer.is_valid());
        assert!(!peer.is_authenticated()); // No node associated
    }

    #[test]
    fn test_packed() {
        let pk = Id::random();
        let seq = 1;
        let nodeid = Some(Id::random());
        let node_sig = Some(crate::random_bytes(64));
        let sig = crate::random_bytes(64);
        let fingerprint = 12345;
        let endpoint = "tcp://1.2.3.4:9000".to_string();
        let extra = Some(vec![1, 2, 3]);

        let peer = PeerInfo::packed(
            pk.clone(),
            seq,
            nodeid.clone(),
            node_sig.clone(),
            sig.clone(),
            fingerprint,
            endpoint.clone(),
            extra.clone()
        );

        assert_eq!(peer.id(), &pk);
        assert_eq!(peer.sequence_number(), seq);
        assert_eq!(peer.nodeid(), nodeid.as_ref());
        assert_eq!(peer.node_signature(), node_sig.as_deref());
        assert_eq!(peer.signature(), &sig);
        assert_eq!(peer.fingerprint(), fingerprint);
        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.extra_data(), extra.as_deref());

        assert!(!peer.has_private_key());
    }

    #[test]
    fn test_serde_simple() {
        let endpoint = "https://example.com:8080";
        let kp = KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .build()
            .expect("Failed to build peer info");

        let ser = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
        let des: PeerInfo = serde_cbor::from_slice(&ser).expect("Failed to deserialize PeerInfo");

        assert_eq!(peer.id(), des.id());
        assert_eq!(peer.endpoint(), des.endpoint());
        assert_eq!(peer.signature(), des.signature());
        assert_eq!(des.sequence_number(), 0);
        assert_eq!(des.fingerprint(), 0);
        assert_eq!(des.nodeid(), None);
        assert_eq!(des.node_signature(), None);
        assert_eq!(des.extra_data(), None);
        assert_eq!(des.has_private_key(), false);

        assert!(des.is_valid());
    }

    #[test] // case6
    fn test_serde_full() {
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
        let peer = rc.expect("Failed to create a Peer");

        let ser = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
        let des: PeerInfo = serde_cbor::from_slice(&ser).expect("Failed to deserialize PeerInfo");

        assert_eq!(peer.id(), des.id());
        assert_eq!(des.private_key(), None);
        assert_eq!(peer.nodeid(), des.nodeid());
        assert_eq!(peer.node_signature(), des.node_signature());
        assert_eq!(peer.is_authenticated(), true);
        assert_eq!(peer.sequence_number(), des.sequence_number());
        assert_eq!(peer.endpoint(), des.endpoint());
        assert_eq!(peer.fingerprint(), des.fingerprint());
    }

    #[test]
    fn test_serde_with_extra() {
        let endpoint = "https://example.com:8080";
        let extra = vec![0u8, 1, 2, 3, 255];
        let kp = KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .with_extra(&extra)
            .with_sequence_number(7)
            .with_fingerprint(42)
            .build()
            .expect("Failed to build peer info");

        let encoded = serde_cbor::to_vec(&peer)
            .expect("Failed to serialize PeerInfo");
        let decoded: PeerInfo = serde_cbor::from_slice(&encoded)
            .expect("Failed to deserialize PeerInfo");

        let json = serde_json::to_string(&peer)
            .expect("Failed to serialize PeerInfo");
        println!("json: {}", json);

        assert!(peer.has_private_key());
        assert_eq!(peer.id(), decoded.id());
        assert_eq!(peer.endpoint(), decoded.endpoint());
        assert_eq!(peer.signature(), decoded.signature());
        assert_eq!(decoded.sequence_number(), 7);
        assert_eq!(decoded.fingerprint(), 42);
        assert_eq!(decoded.extra_data(), Some(extra.as_slice()));
        assert_eq!(decoded.has_private_key(), false);
        assert!(decoded.is_valid());
    }

    #[test]
    fn test_serde_json() {
        let endpoint = "https://example.com:8080";
        let kp = KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .with_sequence_number(5)
            .with_fingerprint(7)
            .build()
            .expect("Failed to build peer info");

        let json = serde_json::to_string(&peer)
            .expect("Failed to serialize PeerInfo to JSON");
        println!("JSON: {}", json);
        let decoded: PeerInfo = serde_json::from_str(&json)
            .expect("Failed to deserialize PeerInfo from JSON");

        assert_eq!(peer.id(), decoded.id());
        assert_eq!(peer.endpoint(), decoded.endpoint());
        assert_eq!(peer.signature(), decoded.signature());
        assert_eq!(decoded.sequence_number(), 5);
        assert_eq!(decoded.fingerprint(), 7);
        assert!(decoded.is_valid());
    }

    #[test]
    fn test_serde_rejects_invalid() {
        let endpoint = "https://example.com:8080";
        let kp = KeyPair::random();
        let valid = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .build()
            .expect("Failed to build peer info");
        assert!(valid.is_valid());

        // Pack the same peer but tamper with the endpoint so the signature no longer verifies.
        let invalid = PeerInfo::packed(
            valid.id().clone(),
            valid.sequence_number(),
            None,
            None,
            valid.signature().to_vec(),
            valid.fingerprint(),
            "https://attacker.com:8080".to_string(),
            None,
        );
        assert!(!invalid.is_valid());

        // Serialization produces bytes, but deserializing the invalid peer must fail.
        let encoded = serde_cbor::to_vec(&invalid)
            .expect("Serialization of an invalid PeerInfo should succeed");
        let result: Result<PeerInfo, _> = serde_cbor::from_slice(&encoded);
        assert!(result.is_err(), "Deserializing an invalid PeerInfo should fail");
    }

    #[test]
    fn test_serde_rejects_empty_endpoint() {
        let pk = Id::random();
        let sig = crate::random_bytes(64);

        let invalid = PeerInfo::packed(
            pk, 0, None, None, sig, 0, String::new(), None
        );

        let encoded = serde_cbor::to_vec(&invalid)
            .expect("Failed to serialize invalid PeerInfo");
        let result: Result<PeerInfo, _> = serde_cbor::from_slice(&encoded);
        assert!(result.is_err(), "Deserializing a PeerInfo with empty endpoint should fail");
    }
}
