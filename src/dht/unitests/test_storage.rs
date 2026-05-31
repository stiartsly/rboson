use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serial_test::serial;

use crate::{
    CryptoIdentity,
    random_bytes,
    Id,
    PeerInfo,
    Value,
    ValueBuilder,
    SignedBuilder,
    EncryptedBuilder,
    signature::KeyPair,
};
use crate::dht::storage::{
    data_storage::DataStorage,
    sqlite_storage::SqliteStorage,
};

fn open_storage(path: &str) -> SqliteStorage {
    let mut s = SqliteStorage::new();
    s.open(path).unwrap_or_else(|e| panic!("Failed to open '{}': {}", path, e));
    s
}

fn new_db_path() -> String {
    let random_suffix = format!("{:016x}", rand::random::<u64>());
    format!("/tmp/ts_{}.db", random_suffix)
}

fn remove_db(path: &str) {
    let _ = fs::remove_file(path);
}

fn make_value() -> Value {
    let rc = ValueBuilder::new(&random_bytes(32))
        .build()
        ;
    assert!(rc.is_ok());
    rc.unwrap()
}

fn make_signed_value(kp: KeyPair, expected_seq: i32) -> Value {
    let rc = SignedBuilder::new(&random_bytes(32))
        .with_keypair(&kp)
        .with_sequence_number(expected_seq)
        .build()
        ;
    assert!(rc.is_ok());
    rc.unwrap()
}

fn make_encrypted_value(recipient: KeyPair, expected_seq: i32) -> Value {
    let recipient = Id::from(recipient.public_key());
    let rc = EncryptedBuilder::new(&random_bytes(32), &recipient)
        .with_sequence_number(expected_seq)
        .build()
        ;
    assert!(rc.is_ok());
    rc.unwrap()
}

fn assert_value_roundtrip(actual: &Value, expected: &Value) {
    assert_eq!(actual.id(), expected.id());
    assert_eq!(actual.sequence_number(), expected.sequence_number());
    assert_eq!(actual.public_key(), expected.public_key());
    assert_eq!(actual.recipient(), expected.recipient());
    assert_eq!(actual.nonce(), expected.nonce());
    assert_eq!(actual.signature(), expected.signature());
    assert_eq!(actual.data(), expected.data());
}

fn make_peer(endpoint: &str, fingerprint: u64) -> PeerInfo {
    let rc = PeerInfo::builder(endpoint)
        .with_fingerprint(fingerprint)
        .build()
        ;
    assert!(rc.is_ok());
    rc.unwrap()
}

fn make_peer_with_key(kp: KeyPair, endpoint: &str, fingerprint: u64, seq: i32) -> PeerInfo {
    let rc = PeerInfo::builder(endpoint)
        .with_key(kp)
        .with_fingerprint(fingerprint)
        .with_sequence_number(seq)
        .build()
        ;
    assert!(rc.is_ok());
    rc.unwrap()
}

fn make_authenticated_peer(endpoint: &str, fingerprint: u64) -> (PeerInfo, Id) {
    let node_identity = Arc::new(Mutex::new(CryptoIdentity::new()));
    let node_id = node_identity.lock().unwrap().id().clone();
    let rc = PeerInfo::builder(endpoint)
        .with_fingerprint(fingerprint)
        .with_node(node_identity)
        .build()
        ;
    assert!(rc.is_ok());
    let peer = rc.unwrap();
    (peer, node_id)
}

fn assert_peer_roundtrip(actual: &PeerInfo, expected: &PeerInfo) {
    assert_eq!(actual.id(), expected.id());
    assert_eq!(actual.fingerprint(), expected.fingerprint());
    assert_eq!(actual.sequence_number(), expected.sequence_number());
    assert_eq!(actual.nodeid(), expected.nodeid());
    assert_eq!(actual.node_signature(), expected.node_signature());
    assert_eq!(actual.signature(), expected.signature());
    assert_eq!(actual.endpoint(), expected.endpoint());
    assert_eq!(actual.extra_data(), expected.extra_data());
}

#[test]
#[serial]
fn test_open_and_close() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = SqliteStorage::new();
    assert!(s.open(&path).is_ok());
    assert!(s.initialize(Duration::from_secs(3600), Duration::from_secs(7200)).is_ok());
    assert!(s.close().is_ok());

    remove_db(&path);
}

#[test]
#[serial]
fn test_retain() {
    let path = new_db_path();
    remove_db(&path);

    let value = make_value();
    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());
    let rc = s.put_value(value.clone(), None);
    assert!(rc.is_ok());
    let rc = s.close();
    assert!(rc.is_ok());

    let s = open_storage(&path);
    let rc = s.get_value(&value.id());
    assert!(rc.is_ok());
    let fetched = rc.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap(), value);

    remove_db(&path);
}

#[test]
#[serial]
fn test_value() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());

    let immutable = make_value();
    let keypair = KeyPair::random();
    let expected_seq1 = 42;
    let signed = make_signed_value(keypair.clone(), expected_seq1);

    let expected_seq2 = 99;
    let encrypted = make_encrypted_value(keypair, expected_seq2);

    assert!(s.put_value(immutable.clone(), None).is_ok());
    assert!(s.put_value(signed.clone(), Some(true)).is_ok());
    assert!(s.put_value(encrypted.clone(), None).is_ok());

    let rc = s.get_value(&immutable.id());
    assert!(rc.is_ok());
    let value = rc.unwrap();
    assert!(value.is_some());
    assert_eq!(value.unwrap(), immutable);

    let rc = s.get_value(&signed.id());
    assert!(rc.is_ok());
    let value = rc.unwrap();
    assert!(value.is_some());
    assert_value_roundtrip(&value.unwrap(), &signed);

    let rc = s.get_value(&encrypted.id());
    assert!(rc.is_ok());
    let value = rc.unwrap();
    assert!(value.is_some());
    assert_value_roundtrip(&value.unwrap(), &encrypted);

    assert!(s.update_value_announced_time(&immutable.id()).is_ok());
    assert!(s.remove_value(&immutable.id()).is_ok());
    let rc = s.get_value(&immutable.id());
    assert!(rc.is_ok());
    let value = rc.unwrap();
    assert!(value.is_none());

    remove_db(&path);
}

#[test]
#[serial]
fn test_values() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());

    let values = vec![
        {
            let rc = ValueBuilder::new(&random_bytes(16)).build();
            assert!(rc.is_ok());
            rc.unwrap()
        },
        {
            let rc = SignedBuilder::new(&random_bytes(16)).build();
            assert!(rc.is_ok());
            rc.unwrap()
        },
        {
            let rc = ValueBuilder::new(&random_bytes(16)).build();
            assert!(rc.is_ok());
            rc.unwrap()
        },
    ];

    for value in &values {
        let rc = s.put_value(value.clone(), None);
        assert!(rc.is_ok());
    }

    let rc = s.get_values();
    assert!(rc.is_ok());
    let all = rc.unwrap();
    assert_eq!(all.len(), values.len());

    for expected in &values {
        let actual = all.iter().find(|value| value.id() == expected.id());
        assert!(actual.is_some(), "missing value {}", expected.id());
        assert_value_roundtrip(actual.unwrap(), expected);
    }

    remove_db(&path);
}

#[test]
#[serial]
fn test_values_with_expected_seq() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());

    let keypair = KeyPair::random();
    let rc = SignedBuilder::new(&random_bytes(16))
        .with_keypair(&keypair)
        .with_sequence_number(3)
        .build()
        ;
    assert!(rc.is_ok());
    let low_seq = rc.unwrap();
    let rc = SignedBuilder::new(&random_bytes(16))
        .with_keypair(&keypair)
        .with_sequence_number(11)
        .build()
        ;
    assert!(rc.is_ok());
    let high_seq = rc.unwrap();

    assert_eq!(
        low_seq.id(),
        high_seq.id(),
        "same keypair must produce same value id"
    );

    let rc = s.put_value(low_seq, None);
    assert!(rc.is_ok());
    let rc = s.put_value(high_seq.clone(), None);
    assert!(rc.is_ok());

    let rc = s.get_value(&high_seq.id());
    assert!(rc.is_ok());
    let fetched = rc.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    let expected_seq = 10;
    assert!(fetched.sequence_number() >= expected_seq);
    assert_eq!(fetched.sequence_number(), high_seq.sequence_number());
    assert_value_roundtrip(&fetched, &high_seq);

    remove_db(&path);
}

#[test]
#[serial]
fn test_peer() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());

    let peer = make_peer("10.0.0.1:9000", 100);
    let (authenticated_peer, node_id) = make_authenticated_peer("10.0.0.2:9000", 200);

    assert!(s.put_peer(peer.clone(), None).is_ok());
    assert!(s.put_peer(authenticated_peer.clone(), Some(true)).is_ok());

    let rc = s.get_peer(peer.id(), peer.fingerprint());
    assert!(rc.is_ok());
    let got_peer = rc.unwrap();
    assert!(got_peer.is_some());
    assert_peer_roundtrip(&got_peer.unwrap(), &peer);

    let rc = s.get_peer(authenticated_peer.id(), authenticated_peer.fingerprint());
    assert!(rc.is_ok());
    let authenticated_peer_value = rc.unwrap();
    assert!(authenticated_peer_value.is_some());
    assert_peer_roundtrip(&authenticated_peer_value.unwrap(), &authenticated_peer);

    let rc = s.get_peers_authenticated_by(authenticated_peer.id(), &node_id);
    assert!(rc.is_ok());
    let peers = rc.unwrap();
    assert_eq!(peers.len(), 1);
    assert_peer_roundtrip(&peers[0], &authenticated_peer);

    let wrong_node = CryptoIdentity::new().id().clone();
    let rc = s.get_peers_authenticated_by(authenticated_peer.id(), &wrong_node);
    assert!(rc.is_ok());
    let peers = rc.unwrap();
    assert!(peers.is_empty());

    assert!(s.update_peer_announced_time(peer.id(), peer.fingerprint()).is_ok());
    assert!(s.remove_peer(peer.id(), peer.fingerprint()).is_ok());
    let rc = s.get_peer(peer.id(), peer.fingerprint());
    assert!(rc.is_ok());
    let removed_peer = rc.unwrap();
    assert!(removed_peer.is_none());

    remove_db(&path);
}

#[test]
#[serial]
fn test_peers() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());

    let keypair = KeyPair::random();
    let p1 = make_peer_with_key(keypair.clone(), "10.0.0.1:9100", 1, 0);
    let p2 = make_peer_with_key(keypair.clone(), "10.0.0.2:9100", 2, 5);
    let p3 = make_peer("10.0.0.3:9100", 3);

    let rc = s.put_peers(vec![p1.clone(), p2.clone(), p3.clone()]);
    assert!(rc.is_ok());

    let rc = s.get_peers(p1.id());
    assert!(rc.is_ok());
    let peers = rc.unwrap();
    assert_eq!(peers.len(), 2);
    for expected in [&p1, &p2] {
        let actual = peers.iter().find(|peer| peer.fingerprint() == expected.fingerprint());
        assert!(actual.is_some(), "missing peer {}", expected.fingerprint());
        assert_peer_roundtrip(actual.unwrap(), expected);
    }

    let rc = s.get_peers_all();
    assert!(rc.is_ok());
    let all = rc.unwrap();
    assert_eq!(all.len(), 3);

    assert!(s.remove_peers(p1.id()).is_ok());
    let rc = s.get_peers(p1.id());
    assert!(rc.is_ok());
    let peers = rc.unwrap();
    assert!(peers.is_empty());

    remove_db(&path);
}

#[test]
#[serial]
fn test_peers_with_expected_seq() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::from_secs(3600), Duration::from_secs(7200));
    assert!(rc.is_ok());

    let keypair = KeyPair::random();
    let low_seq = make_peer_with_key(keypair.clone(), "10.0.1.1:9200", 11, 3);
    let high_seq = make_peer_with_key(keypair, "10.0.1.2:9200", 22, 11);

    assert_eq!(low_seq.id(), high_seq.id(), "same keypair must produce same peer id");

    let rc = s.put_peer(low_seq, None);
    assert!(rc.is_ok());
    let rc = s.put_peer(high_seq.clone(), None);
    assert!(rc.is_ok());

    let rc = s.get_peers_with_expected_seq(high_seq.id(), 10, 10);
    assert!(rc.is_ok());
    let peers = rc.unwrap();
    assert_eq!(peers.len(), 1);
    assert!(peers[0].sequence_number() >= 10);
    assert_peer_roundtrip(&peers[0], &high_seq);

    remove_db(&path);
}

#[test]
#[serial]
fn test_purge() {
    let path = new_db_path();
    remove_db(&path);

    let mut s = open_storage(&path);
    let rc = s.initialize(Duration::ZERO, Duration::ZERO);
    assert!(rc.is_ok());

    let volatile_value = make_value();
    let persistent_value = make_signed_value(KeyPair::random(), 7);
    let volatile_peer = make_peer("10.0.2.1:9300", 31);
    let persistent_peer = make_peer("10.0.2.2:9300", 32);

    let rc = s.put_value(volatile_value.clone(), None);
    assert!(rc.is_ok());
    let rc = s.put_value(persistent_value.clone(), Some(true));
    assert!(rc.is_ok());
    let rc = s.put_peer(volatile_peer.clone(), None);
    assert!(rc.is_ok());
    let rc = s.put_peer(persistent_peer.clone(), Some(true));
    assert!(rc.is_ok());

    assert!(s.purge().is_ok());

    let rc = s.get_value(&volatile_value.id());
    assert!(rc.is_ok());
    let value = rc.unwrap();
    assert!(value.is_none());
    let rc = s.get_value(&persistent_value.id());
    assert!(rc.is_ok());
    let value = rc.unwrap();
    assert!(value.is_some());
    assert_value_roundtrip(&value.unwrap(), &persistent_value);

    let rc = s.get_peer(volatile_peer.id(), volatile_peer.fingerprint());
    assert!(rc.is_ok());
    let peer = rc.unwrap();
    assert!(peer.is_none());
    let rc = s.get_peer(persistent_peer.id(), persistent_peer.fingerprint());
    assert!(rc.is_ok());
    let peer = rc.unwrap();
    assert!(peer.is_some());
    assert_peer_roundtrip(
        &peer.unwrap(),
        &persistent_peer,
    );

    remove_db(&path);
}
