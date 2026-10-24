// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::{
    signature,
    signature::KeyPair,
    CryptoIdentity,
    Id,
    PeerBuilder,
    PeerInfo
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_builder() {
        let endpoint = "https://example.com:8080";
        let key = KeyPair::random();
        let id = Id::from(key.public_key());

        let peer = PeerBuilder::new(endpoint)
            .with_key(key.clone())
            .build()
            .expect("Failed to build peer info");

        assert_eq!(peer.endpoint(), endpoint);
        assert_eq!(peer.id(), &id);
        assert!(peer.has_private_key());
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
            extra.clone(),
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
    fn test_serialize_simple() {
        let endpoint = "https://example.com:8080";
        let key = KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(key.clone())
            .build()
            .expect("Failed to build peer info");

        let encoded = serde_cbor::to_vec(&peer)
            .expect("Failed to serialize PeerInfo");
        let decoded = serde_cbor::from_slice::<PeerInfo>(&encoded)
            .expect("Failed to deserialize PeerInfo");

        assert_eq!(peer.id(), decoded.id());
        assert_eq!(peer.endpoint(), decoded.endpoint());
        assert_eq!(peer.signature(), decoded.signature());
        assert_eq!(decoded.sequence_number(), 0);
        assert_eq!(decoded.fingerprint(), peer.fingerprint());
        assert_eq!(decoded.nodeid(), None);
        assert_eq!(decoded.node_signature(), None);
        assert_eq!(decoded.extra_data(), None);

        assert!(!decoded.has_private_key());
        assert!(decoded.is_valid());
    }

    #[test] // case6
    fn test_serialize_full() {
        let endpoint = "http://localhost:8080";
        let node_kp = signature::KeyPair::random();
        let node = CryptoIdentity::from(node_kp);
        let peer_kp = signature::KeyPair::random();
        let rc = PeerBuilder::new(endpoint)
            .with_key(peer_kp.clone())
            .with_node(&node)
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
        assert!(des.is_valid());
    }

    #[test]
    fn test_serialize_with_extra() {
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

        let encoded = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
        let decoded: PeerInfo =
            serde_cbor::from_slice(&encoded).expect("Failed to deserialize PeerInfo");

        let json = serde_json::to_string(&peer).expect("Failed to serialize PeerInfo");
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
    fn test_serialize_json() {
        let endpoint = "https://example.com:8080";
        let kp = KeyPair::random();
        let peer = PeerBuilder::new(endpoint)
            .with_key(kp.clone())
            .with_sequence_number(5)
            .with_fingerprint(7)
            .build()
            .expect("Failed to build peer info");

        let json = serde_json::to_string(&peer).expect("Failed to serialize PeerInfo to JSON");
        println!("JSON: {}", json);
        let decoded: PeerInfo =
            serde_json::from_str(&json).expect("Failed to deserialize PeerInfo from JSON");

        assert_eq!(peer.id(), decoded.id());
        assert_eq!(peer.endpoint(), decoded.endpoint());
        assert_eq!(peer.signature(), decoded.signature());
        assert_eq!(decoded.sequence_number(), 5);
        assert_eq!(decoded.fingerprint(), 7);
        assert!(decoded.is_valid());
    }

    #[test]
    fn test_serialize_invalid_peer() {
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

        let encoded = serde_cbor::to_vec(&invalid)
            .expect("Serialization of an invalid PeerInfo should succeed");
        let result: Result<PeerInfo, _> = serde_cbor::from_slice(&encoded);
        assert!(
            result.is_err(),
            "Deserializing an invalid PeerInfo should fail"
        );
    }

    #[test]
    fn test_serialize_peer_with_empty_endpoint() {
        let pk = Id::random();
        let sig = crate::random_bytes(64);

        let invalid = PeerInfo::packed(pk, 0, None, None, sig, 0, String::new(), None);

        let encoded = serde_cbor::to_vec(&invalid)
            .expect("Failed to serialize invalid PeerInfo");

        let rc: Result<PeerInfo, _> = serde_cbor::from_slice(&encoded);
        assert!(rc.is_err());
    }

    fn signed_peer(node: Option<&CryptoIdentity>) -> PeerInfo {
        let builder = PeerBuilder::new("tcp://1.2.3.4:9000")
            .with_fingerprint(7);

        let builder = if let Some(node) = node {
            builder.with_node(node)
        } else {
            builder
        };
        builder.build().unwrap()
    }

    fn repack(peer: &PeerInfo) -> PeerInfo {
        PeerInfo::packed(
            peer.id().clone(),
            peer.sequence_number(),
            peer.nodeid().cloned(),
            peer.node_signature().map(|s| s.to_vec()),
            peer.signature().to_vec(),
            peer.fingerprint(),
            peer.endpoint().to_string(),
            peer.extra_data().map(|e| e.to_vec()),
        )
    }

    #[test]
    fn test_packed_roundtrip_is_valid_and_keyless() {
        let node = CryptoIdentity::random();
        for peer in [signed_peer(None), signed_peer(Some(&node))] {
            assert!(peer.has_private_key());
            let packed = repack(&peer);
            assert!(!packed.has_private_key());
            assert!(packed.is_valid());
            assert_eq!(packed, peer);
            assert_eq!(packed.is_authenticated(), peer.is_authenticated());
        }
    }

    #[test]
    fn test_packed_does_not_verify() {
        let packed = PeerInfo::packed(
            Id::random(), 0, None, None, vec![0; 64], 0, "tcp://x:1".into(), None,
        );
        assert!(!packed.is_valid());
    }

    #[test]
    fn test_digest() {
        let pk = [1u8; 32];
        let base = PeerInfo::digest(&pk, 1, None, None, 2, "e", None);
        assert_eq!(base.len(), 32);
        assert_eq!(base, PeerInfo::digest(&pk, 1, None, None, 2, "e", None));

        assert_ne!(base, PeerInfo::digest(&pk, 2, None, None, 2, "e", None));
        assert_ne!(base, PeerInfo::digest(&pk, 1, None, None, 3, "e", None));
        assert_ne!(base, PeerInfo::digest(&pk, 1, None, None, 2, "f", None));
        assert_ne!(base, PeerInfo::digest(&pk, 1, None, None, 2, "e", Some(&[1])));

        let nid = [9u8; 32];
        let sig = [8u8; 64];
        let with_node = PeerInfo::digest(&pk, 1, Some(&nid), Some(&sig), 2, "e", None);
        assert_ne!(base, with_node);
        // Without a node signature the node id is ignored.
        assert_eq!(base, PeerInfo::digest(&pk, 1, Some(&nid), None, 2, "e", None));
    }

    #[test]
    fn test_node_digest() {
        let pk = [1u8; 32];
        let nid = [2u8; 32];
        let base = PeerInfo::node_digest(&pk, &nid, 5, 1);
        assert_eq!(base.len(), 32);
        assert_eq!(base, PeerInfo::node_digest(&pk, &nid, 5, 1));
        assert_ne!(base, PeerInfo::node_digest(&[3u8; 32], &nid, 5, 1));
        assert_ne!(base, PeerInfo::node_digest(&pk, &[3u8; 32], 5, 1));
        assert_ne!(base, PeerInfo::node_digest(&pk, &nid, 6, 1));
        assert_ne!(base, PeerInfo::node_digest(&pk, &nid, 5, 2));
    }

    #[test]
    fn test_without_private_key() {
        let peer = signed_peer(None);
        let stripped = peer.without_private_key();
        assert!(!stripped.has_private_key());
        assert!(stripped.is_valid());
        assert_eq!(stripped, peer);
        assert!(!stripped.without_private_key().has_private_key());
    }

    #[test]
    fn test_update() {
        let node = CryptoIdentity::random();
        let peer = signed_peer(None);

        assert!(peer.update().build().is_err());

        let updated = peer.update().with_node(&node).build().unwrap();
        assert!(updated.is_valid());
        assert!(updated.is_authenticated());
        assert_eq!(updated.sequence_number(), peer.sequence_number() + 1);
        assert_eq!(updated.fingerprint(), peer.fingerprint());
        assert_eq!(updated.id(), peer.id());

        // Authenticated peers cannot be updated again.
        assert!(updated.update().with_node(&node).build().is_err());
    }
}
