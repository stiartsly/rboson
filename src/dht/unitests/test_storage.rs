use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serial_test::serial;

use crate::{
    random_bytes,
    Id,
    Value,
    PeerInfo,
    ValueBuilder,
    SignedBuilder,
    EncryptedBuilder,
    signature::KeyPair,
    CryptoIdentity,
};
use crate::dht::storage::{
    data_storage::DataStorage,
    sqlite_storage::SqliteStorage,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn open_storage(path: &str) -> SqliteStorage {
    let mut s = SqliteStorage::new();
    s.open(path).unwrap_or_else(|e| panic!("Failed to open '{}': {}", path, e));
    s
}

fn remove_db(path: &str) {
    let _ = fs::remove_file(path);
}

/// Build a simple peer with no node-authentication.
fn make_peer(endpoint: &str, fingerprint: u64) -> PeerInfo {
    PeerInfo::builder(endpoint)
        .with_fingerprint(fingerprint)
        .build()
        .expect("PeerBuilder::build")
}

/// Build a peer with an explicit keypair so multiple peers can share the same id.
fn make_peer_with_key(kp: KeyPair, endpoint: &str, fingerprint: u64) -> PeerInfo {
    PeerInfo::builder(endpoint)
        .with_key(kp)
        .with_fingerprint(fingerprint)
        .build()
        .expect("PeerBuilder::build")
}

// ── DataStorage::open / initialize / close ────────────────────────────────────

#[test]
#[serial]
fn test_open_and_close() {
    let path = "/tmp/ts_open_close.db";
    remove_db(path);

    let mut s = SqliteStorage::new();
    assert!(s.open(path).is_ok());
    assert!(s.close().is_ok());

    remove_db(path);
}

#[test]
#[serial]
fn test_initialize() {
    let path = "/tmp/ts_initialize.db";
    remove_db(path);

    let mut s = open_storage(path);
    assert!(s.initialize(Duration::from_secs(3600), Duration::from_secs(7200)).is_ok());
    assert!(s.close().is_ok());

    remove_db(path);
}

#[test]
#[serial]
fn test_close_and_reopen_retains_data() {
    let path = "/tmp/ts_reopen.db";
    remove_db(path);

    // insert a value, then close
    {
        let mut s = open_storage(path);
        let v = ValueBuilder::new(&random_bytes(32)).build().unwrap();
        s.put_value(v, None).unwrap();
        s.close().unwrap();
    }

    // reopen and verify the value is still there
    let s = open_storage(path);
    let all = s.get_values().unwrap();
    assert_eq!(all.len(), 1, "value should survive a close/reopen cycle");

    remove_db(path);
}

// ── DataStorage::put_value / get_value ───────────────────────────────────────

#[test]
#[serial]
fn test_put_and_get_immutable_value() {
    let path = "/tmp/ts_immutable.db";
    remove_db(path);
    let mut s = open_storage(path);

    let data = random_bytes(32);
    let v = ValueBuilder::new(&data).build().unwrap();
    let id = v.id();

    // not present yet
    assert!(s.get_value(&id).unwrap().is_none());

    s.put_value(v.clone(), None).unwrap();

    let fetched = s.get_value(&id).unwrap().expect("value must be present");
    assert_eq!(fetched, v);

    remove_db(path);
}

#[test]
#[serial]
fn test_put_and_get_signed_value() {
    let path = "/tmp/ts_signed.db";
    remove_db(path);
    let mut s = open_storage(path);

    let v = SignedBuilder::new(&random_bytes(32))
        .with_sequence_number(42)
        .build()
        .unwrap();
    let id = v.id();

    s.put_value(v.clone(), Some(true)).unwrap();

    let fetched = s.get_value(&id).unwrap().expect("signed value must be present");
    // Private key is stored but not restored via Value::packed (by design),
    // so we compare the observable fields individually.
    assert_eq!(fetched.id(),              v.id());
    assert_eq!(fetched.sequence_number(), v.sequence_number());
    assert_eq!(fetched.public_key(),      v.public_key());
    assert_eq!(fetched.nonce(),           v.nonce());
    assert_eq!(fetched.signature(),       v.signature());
    assert_eq!(fetched.data(),            v.data());
    assert!(!fetched.has_private_key(),   "sk is not restored from storage");

    remove_db(path);
}

#[test]
#[serial]
fn test_put_and_get_encrypted_value() {
    let path = "/tmp/ts_encrypted.db";
    remove_db(path);
    let mut s = open_storage(path);

    let recipient_kp = KeyPair::random();
    let recipient    = Id::from(recipient_kp.public_key());
    let v = EncryptedBuilder::new(&random_bytes(32), &recipient)
        .with_sequence_number(7)
        .build()
        .unwrap();
    let id = v.id();

    s.put_value(v.clone(), None).unwrap();

    let fetched = s.get_value(&id).unwrap().expect("encrypted value must be present");
    // Private key is stored but not restored via Value::packed (by design).
    assert_eq!(fetched.id(),              v.id());
    assert_eq!(fetched.sequence_number(), v.sequence_number());
    assert_eq!(fetched.public_key(),      v.public_key());
    assert_eq!(fetched.recipient(),       v.recipient());
    assert_eq!(fetched.nonce(),           v.nonce());
    assert_eq!(fetched.signature(),       v.signature());
    assert_eq!(fetched.data(),            v.data());
    assert!(!fetched.has_private_key(),   "sk is not restored from storage");

    remove_db(path);
}

// ── DataStorage::get_values ───────────────────────────────────────────────────

#[test]
#[serial]
fn test_get_values_returns_all() {
    let path = "/tmp/ts_get_values.db";
    remove_db(path);
    let mut s = open_storage(path);

    let v1 = ValueBuilder::new(&random_bytes(16)).build().unwrap();
    let v2 = SignedBuilder::new(&random_bytes(16)).build().unwrap();
    let v3 = ValueBuilder::new(&random_bytes(16)).build().unwrap();

    s.put_value(v1, None).unwrap();
    s.put_value(v2, None).unwrap();
    s.put_value(v3, None).unwrap();

    let all = s.get_values().unwrap();
    assert_eq!(all.len(), 3);

    remove_db(path);
}

#[test]
#[serial]
fn test_get_values_empty_db() {
    let path = "/tmp/ts_values_empty.db";
    remove_db(path);
    let s = open_storage(path);

    let all = s.get_values().unwrap();
    assert!(all.is_empty());

    remove_db(path);
}

// ── DataStorage::update_value_announced_time ─────────────────────────────────

#[test]
#[serial]
fn test_update_value_announced_time() {
    let path = "/tmp/ts_val_announce.db";
    remove_db(path);
    let mut s = open_storage(path);

    let v = ValueBuilder::new(&random_bytes(16)).build().unwrap();
    let id = v.id();
    s.put_value(v, None).unwrap();

    // idempotent – must succeed without error
    assert!(s.update_value_announced_time(&id).is_ok());
    assert!(s.update_value_announced_time(&id).is_ok());

    remove_db(path);
}

// ── DataStorage::remove_value ─────────────────────────────────────────────────

#[test]
#[serial]
fn test_remove_value() {
    let path = "/tmp/ts_remove_val.db";
    remove_db(path);
    let mut s = open_storage(path);

    let v = ValueBuilder::new(&random_bytes(16)).build().unwrap();
    let id = v.id();
    s.put_value(v, None).unwrap();
    assert!(s.get_value(&id).unwrap().is_some());

    s.remove_value(&id).unwrap();
    assert!(s.get_value(&id).unwrap().is_none());

    // removing an absent value should not error
    assert!(s.remove_value(&id).is_ok());

    remove_db(path);
}

// ── DataStorage::put_peer / get_peer ─────────────────────────────────────────

#[test]
#[serial]
fn test_put_and_get_peer() {
    let path = "/tmp/ts_put_peer.db";
    remove_db(path);
    let mut s = open_storage(path);

    let peer = make_peer("192.168.1.1:8080", 1001);
    let id = peer.id().clone();
    let fp = peer.fingerprint();

    // not present yet
    assert!(s.get_peer(&id, fp).unwrap().is_none());

    s.put_peer(peer.clone(), None).unwrap();

    let fetched = s.get_peer(&id, fp).unwrap().expect("peer must be present");
    assert_eq!(fetched.id(), &id);
    assert_eq!(fetched.fingerprint(), fp);
    assert_eq!(fetched.endpoint(), peer.endpoint());

    remove_db(path);
}

#[test]
#[serial]
fn test_put_peer_persistent_flag() {
    let path = "/tmp/ts_peer_persistent.db";
    remove_db(path);
    let mut s = open_storage(path);

    let peer = make_peer("10.0.0.1:9000", 500);
    s.put_peer(peer.clone(), Some(true)).unwrap();

    let fetched = s.get_peer(peer.id(), peer.fingerprint()).unwrap().unwrap();
    assert_eq!(fetched.id(), peer.id());

    remove_db(path);
}

// ── DataStorage::put_peers ────────────────────────────────────────────────────

#[test]
#[serial]
fn test_put_peers_bulk() {
    let path = "/tmp/ts_put_peers.db";
    remove_db(path);
    let mut s = open_storage(path);

    let p1 = make_peer("10.0.0.1:8080", 1);
    let p2 = make_peer("10.0.0.2:8080", 2);
    let p3 = make_peer("10.0.0.3:8080", 3);

    s.put_peers(vec![p1.clone(), p2.clone(), p3.clone()]).unwrap();

    assert!(s.get_peer(p1.id(), p1.fingerprint()).unwrap().is_some());
    assert!(s.get_peer(p2.id(), p2.fingerprint()).unwrap().is_some());
    assert!(s.get_peer(p3.id(), p3.fingerprint()).unwrap().is_some());

    remove_db(path);
}

// ── DataStorage::get_peers ────────────────────────────────────────────────────

#[test]
#[serial]
fn test_get_peers_by_id() {
    let path = "/tmp/ts_get_peers.db";
    remove_db(path);
    let mut s = open_storage(path);

    // Two peers sharing the same signing keypair (→ same id) but different fingerprints
    let kp = KeyPair::random();
    let p1 = make_peer_with_key(kp.clone(), "10.0.0.1:9000", 100);
    let p2 = make_peer_with_key(kp.clone(), "10.0.0.1:9001", 200);
    assert_eq!(p1.id(), p2.id(), "same keypair must produce same id");

    let id = p1.id().clone();
    s.put_peer(p1, None).unwrap();
    s.put_peer(p2, None).unwrap();

    let peers = s.get_peers(&id).unwrap();
    assert_eq!(peers.len(), 2);

    // a different id should return nothing
    let other_id = Id::random();
    assert!(s.get_peers(&other_id).unwrap().is_empty());

    remove_db(path);
}

// ── DataStorage::get_peers_with_expected_seq ──────────────────────────────────

#[test]
#[serial]
fn test_get_peers_with_expected_seq() {
    let path = "/tmp/ts_peers_seq.db";
    remove_db(path);
    let mut s = open_storage(path);

    let kp = KeyPair::random();
    let p_lo = make_peer_with_key(kp.clone(), "10.0.0.1:9000", 1);  // seq = 0 (default)
    let p_hi = PeerInfo::builder("10.0.0.1:9001")
        .with_key(kp.clone())
        .with_fingerprint(2)
        .with_sequence_number(10)
        .build()
        .unwrap();
    let id = p_lo.id().clone();

    s.put_peer(p_lo.clone(), None).unwrap();
    s.put_peer(p_hi.clone(), None).unwrap();

    // seq >= 5 → only p_hi
    let result = s.get_peers_with_expected_seq(&id, 5, 100).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fingerprint(), 2);

    // seq >= 0 → both
    let result_all = s.get_peers_with_expected_seq(&id, 0, 100).unwrap();
    assert_eq!(result_all.len(), 2);

    // limit = 1 → at most 1 result
    let result_limited = s.get_peers_with_expected_seq(&id, 0, 1).unwrap();
    assert_eq!(result_limited.len(), 1);

    remove_db(path);
}

// ── DataStorage::get_peers_authenticated_by ───────────────────────────────────

#[test]
#[serial]
fn test_get_peers_authenticated_by_unauthenticated() {
    let path = "/tmp/ts_peers_unauth.db";
    remove_db(path);
    let mut s = open_storage(path);

    // Peer without a node identity is NOT authenticated
    let peer = make_peer("10.0.0.1:9000", 77);
    let id = peer.id().clone();
    s.put_peer(peer, None).unwrap();

    let any_node_id = Id::random();
    let result = s.get_peers_authenticated_by(&id, &any_node_id).unwrap();
    assert!(result.is_empty());

    remove_db(path);
}

#[test]
#[serial]
fn test_get_peers_authenticated_by_authenticated() {
    let path = "/tmp/ts_peers_auth.db";
    remove_db(path);
    let mut s = open_storage(path);

    // Build a peer authenticated by a CryptoIdentity node
    let node_identity = Arc::new(Mutex::new(CryptoIdentity::new()));
    let node_id       = node_identity.lock().unwrap().id().clone();

    let peer = PeerInfo::builder("10.0.0.2:9000")
        .with_fingerprint(99)
        .with_node(node_identity)
        .build()
        .unwrap();
    let peer_id = peer.id().clone();
    assert!(peer.is_authenticated(), "peer must carry a node signature");

    s.put_peer(peer.clone(), None).unwrap();

    // query by the correct node id → found
    let result = s.get_peers_authenticated_by(&peer_id, &node_id).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fingerprint(), 99);

    // query by a different node id → not found
    let other_node = Id::random();
    let result_other = s.get_peers_authenticated_by(&peer_id, &other_node).unwrap();
    assert!(result_other.is_empty());

    remove_db(path);
}

// ── DataStorage::get_peers_all ────────────────────────────────────────────────

#[test]
#[serial]
fn test_get_peers_all() {
    let path = "/tmp/ts_peers_all.db";
    remove_db(path);
    let mut s = open_storage(path);

    let p1 = make_peer("10.0.0.1:8080", 1);
    let p2 = make_peer("10.0.0.2:8080", 2);
    let p3 = make_peer("10.0.0.3:8080", 3);

    s.put_peer(p1, None).unwrap();
    s.put_peer(p2, None).unwrap();
    s.put_peer(p3, None).unwrap();

    let all = s.get_peers_all().unwrap();
    assert_eq!(all.len(), 3);

    remove_db(path);
}

#[test]
#[serial]
fn test_get_peers_all_empty_db() {
    let path = "/tmp/ts_peers_all_empty.db";
    remove_db(path);
    let s = open_storage(path);

    assert!(s.get_peers_all().unwrap().is_empty());

    remove_db(path);
}

// ── DataStorage::update_peer_announced_time ───────────────────────────────────

#[test]
#[serial]
fn test_update_peer_announced_time() {
    let path = "/tmp/ts_peer_announce.db";
    remove_db(path);
    let mut s = open_storage(path);

    let peer = make_peer("10.0.0.1:8080", 5);
    let id = peer.id().clone();
    let fp = peer.fingerprint();
    s.put_peer(peer, None).unwrap();

    // idempotent – must succeed without error
    assert!(s.update_peer_announced_time(&id, fp).is_ok());
    assert!(s.update_peer_announced_time(&id, fp).is_ok());

    remove_db(path);
}

// ── DataStorage::remove_peer ──────────────────────────────────────────────────

#[test]
#[serial]
fn test_remove_peer() {
    let path = "/tmp/ts_remove_peer.db";
    remove_db(path);
    let mut s = open_storage(path);

    let peer = make_peer("10.0.0.1:8080", 42);
    let id = peer.id().clone();
    let fp = peer.fingerprint();
    s.put_peer(peer, None).unwrap();
    assert!(s.get_peer(&id, fp).unwrap().is_some());

    s.remove_peer(&id, fp).unwrap();
    assert!(s.get_peer(&id, fp).unwrap().is_none());

    // removing a non-existent peer must not error
    assert!(s.remove_peer(&id, fp).is_ok());

    remove_db(path);
}

// ── DataStorage::remove_peers ─────────────────────────────────────────────────

#[test]
#[serial]
fn test_remove_peers_by_id() {
    let path = "/tmp/ts_remove_peers.db";
    remove_db(path);
    let mut s = open_storage(path);

    let kp = KeyPair::random();
    let p1 = make_peer_with_key(kp.clone(), "10.0.0.1:8080", 1);
    let p2 = make_peer_with_key(kp.clone(), "10.0.0.2:8080", 2);
    let p3 = make_peer_with_key(kp.clone(), "10.0.0.3:8080", 3);
    let id = p1.id().clone();

    s.put_peer(p1, None).unwrap();
    s.put_peer(p2, None).unwrap();
    s.put_peer(p3, None).unwrap();
    assert_eq!(s.get_peers(&id).unwrap().len(), 3);

    s.remove_peers(&id).unwrap();
    assert!(s.get_peers(&id).unwrap().is_empty());

    // removing again must not error
    assert!(s.remove_peers(&id).is_ok());

    remove_db(path);
}

// ── DataStorage::purge ────────────────────────────────────────────────────────

#[test]
#[serial]
fn test_purge_removes_expired_records() {
    let path = "/tmp/ts_purge.db";
    remove_db(path);

    let mut s = SqliteStorage::new();
    s.open(path).unwrap();
    // 0-ms expiry → every record is already past its deadline
    s.initialize(Duration::from_millis(0), Duration::from_millis(0)).unwrap();

    let v = ValueBuilder::new(&random_bytes(16)).build().unwrap();
    let v_id = v.id();
    s.put_value(v, None).unwrap();   // persistent = false (default)

    let peer = make_peer("10.0.0.1:8080", 7);
    let p_id = peer.id().clone();
    let p_fp = peer.fingerprint();
    s.put_peer(peer, None).unwrap(); // persistent = false (default)

    s.purge().unwrap();

    assert!(s.get_value(&v_id).unwrap().is_none(),  "expired value must be purged");
    assert!(s.get_peer(&p_id, p_fp).unwrap().is_none(), "expired peer must be purged");

    remove_db(path);
}

#[test]
#[serial]
fn test_purge_keeps_persistent_records() {
    let path = "/tmp/ts_purge_persistent.db";
    remove_db(path);

    let mut s = SqliteStorage::new();
    s.open(path).unwrap();
    // 0-ms expiry, but records are stored as persistent → must survive purge
    s.initialize(Duration::from_millis(0), Duration::from_millis(0)).unwrap();

    let v = ValueBuilder::new(&random_bytes(16)).build().unwrap();
    let v_id = v.id();
    s.put_value(v, Some(true)).unwrap();   // persistent = true

    let peer = make_peer("10.0.0.1:8080", 99);
    let p_id = peer.id().clone();
    let p_fp = peer.fingerprint();
    s.put_peer(peer, Some(true)).unwrap();  // persistent = true

    s.purge().unwrap();

    assert!(s.get_value(&v_id).unwrap().is_some(),  "persistent value must survive purge");
    assert!(s.get_peer(&p_id, p_fp).unwrap().is_some(), "persistent peer must survive purge");

    remove_db(path);
}
