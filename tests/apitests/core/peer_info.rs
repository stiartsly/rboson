use std::sync::{Arc, Mutex};
use rand::RngCore;
use boson::{
    Id,
    PeerInfo,
    PeerBuilder,
    CryptoIdentity,
    signature,
};

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
    let identity = CryptoIdentity::from_keypair(keypair);
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
fn test_with_whole() {
    let endpoint = "http://localhost:8080";
    let node_kp = signature::KeyPair::random();
    let node_identity = CryptoIdentity::from_keypair(node_kp);
    let node = Arc::new(Mutex::new(node_identity));
    let peer_kp = signature::KeyPair::random();
    let mut nonce = vec![0u8; PeerInfo::NONCE_BYTES];
    rand::thread_rng().fill_bytes(&mut nonce);
    let rc = PeerBuilder::new(endpoint)
        .with_key(peer_kp.clone())
        .with_nonce(&nonce)
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
    assert_eq!(peer.nonce(), nonce);
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
    rand::thread_rng().fill_bytes(&mut nonce);
    let rc1 = PeerBuilder::new(endpoint)
        .with_key(kp.clone())
        .with_nonce(&nonce)
        .build();
    let peer1 = rc1.unwrap();

    let rc2 = PeerBuilder::new(endpoint)
        .with_key(kp)
        .with_nonce(&nonce)
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
    let node_identity = CryptoIdentity::from_keypair(node_kp);
    let node = Arc::new(Mutex::new(node_identity));
    let peer_kp = signature::KeyPair::random();
    let mut nonce = vec![0u8; PeerInfo::NONCE_BYTES];
    rand::thread_rng().fill_bytes(&mut nonce);
    let rc = PeerBuilder::new(endpoint)
        .with_key(peer_kp.clone())
        .with_nonce(&nonce)
        .with_node(node.clone())
        .with_sequence_number(101)
        .with_fingerprint(100)
        .build();
    let peer1 = rc.unwrap();

    let rc = PeerBuilder::new(endpoint)
        .with_key(peer_kp.clone())
        .with_nonce(&nonce)
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
    let node_identity = CryptoIdentity::from_keypair(node_kp);
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
