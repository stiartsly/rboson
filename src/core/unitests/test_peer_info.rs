use std::sync::{Arc, Mutex};
use crate::core::{
    Id,
    signature,
    CryptoIdentity,
    PeerInfo,
    PeerBuilder,
    signature::KeyPair,
    unitests::create_random_bytes,
};

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
    let nonce = create_random_bytes(24);
    let seq = 1;
    let nodeid = Some(Id::random());
    let node_sig = Some(create_random_bytes(64));
    let sig = create_random_bytes(64);
    let fingerprint = 12345;
    let endpoint = "tcp://1.2.3.4:9000".to_string();
    let extra = Some(vec![1, 2, 3]);

    let peer = PeerInfo::packed(
        pk.clone(),
        nonce.clone(),
        seq,
        nodeid.clone(),
        node_sig.clone(),
        sig.clone(),
        fingerprint,
        endpoint.clone(),
        extra.clone()
    );

    assert_eq!(peer.id(), &pk);
    assert_eq!(peer.nonce(), &nonce);
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
    assert_eq!(peer.nonce(), des.nonce());
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
    let node_identity = CryptoIdentity::from_keypair(node_kp);
    let node = Arc::new(Mutex::new(node_identity));
    let peer_kp = signature::KeyPair::random();
    let mut nonce = vec![0u8; PeerInfo::NONCE_BYTES];
    rand::fill(&mut nonce);
    let rc = PeerBuilder::new(endpoint)
        .with_key(peer_kp.clone())
        .with_nonce(&nonce)
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

