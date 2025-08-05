use std::{
    fs,
    time::SystemTime,
    collections::HashMap,
};
use serial_test::serial;
use crate::{
    random_bytes,
    Id,
    Value,
    signature::KeyPair,
    PeerInfo,
    PeerBuilder,
    ValueBuilder,
    SignedBuilder,
    EncryptedBuilder
};

use crate::dht::{
    data_storage::DataStorage,
    sqlite_storage::SqliteStorage,
};

fn get_storage() -> (Box<dyn DataStorage>, String) {
    let mut storage = SqliteStorage::new();
    let path = "sqlite.db".to_string();
    match storage.open(&path) {
        Ok(_) => {
            (Box::new(storage), path)
        }
        Err(e) => {
            panic!("opening db error: {}", e);
        }
    }
}
fn remove_storage(path: &str) {
    _ = fs::remove_file(path)
}

#[test]
#[serial]
fn test_value() {
    let (mut db, path) = get_storage();

    // create a new immutable value
    let data = random_bytes(32);
    let value: Value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build value");
    let value_id = value.id();

    // Check the value from storage where it should not exist.
    let result = db.value(&value_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.ok().unwrap().is_some(), false);

    // put value to storage
    let result = db.put_value(&value, Some(0), Some(true), Some(false));
    assert_eq!(result.is_ok(), true);

    // Recheck the value from storage where it should
    // be now available.
    let result = db.value(&value_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().unwrap().is_some(), true);
    assert_eq!(result.ok().unwrap(), Some(value));

    // update value be announced.
    let result = db.update_value_last_announce(&value_id);
    assert_eq!(result.is_ok(), true);

    // query ids
    let result = db.value_ids();
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().unwrap().len(), 1);
    assert_eq!(result.as_ref().ok().unwrap()[0], value_id);

    // remove value
    let result = db.remove_value(&value_id);
    assert_eq!(result.is_ok(), true);

    // Recheck the value from storage where it should
    // no longer be available.
    let result = db.value(&value_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().unwrap().is_some(), false);

    remove_storage(&path);
}

#[test]
#[serial]
fn test_values() {
    let (mut db, path) = get_storage();

    // create a immutable value;
    let data1 = random_bytes(32);
    let value1 = SignedBuilder::new(&data1)
        .build()
        .expect("Failed to build value");
    let value1_id = value1.id();

    // create a signed value;
    let data2 = random_bytes(32);
    let value2 = SignedBuilder::new(&data2)
        .with_sequence_number(55)
        .build()
        .expect("Failed to build value");
    let value2_id = value2.id();

    // create a encrypted value;
    let data2 = random_bytes(32);
    let keypair = KeyPair::random();
    let recipient = Id::from(keypair.public_key());
    let value3 = EncryptedBuilder::new(&data2, &recipient)
        .with_sequence_number(55)
        .build()
        .expect("Failed to build value");
    let value3_id = value3.id();

    // Put them into storage.
    let result = db.put_value(&value1, Some(0), Some(false), Some(false));
    assert_eq!(result.is_ok(), true);
    let result = db.put_value(&value2, Some(55), Some(true), Some(false));
    assert_eq!(result.is_ok(), true);
    let result = db.put_value(&value3, Some(55), Some(true), Some(false));
    assert_eq!(result.is_ok(), true);

    // Check value1
    let result = db.value(&value1_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().is_some(), true);
    assert_eq!(result.ok().unwrap(), Some(value1));

    // Check value2
    let result = db.value(&value2_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().is_some(), true);
    assert_eq!(result.unwrap(), Some(value2.clone()));

    // Check value3
    let result = db.value(&value3_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().is_some(), true);
    assert_eq!(result.unwrap(), Some(value3.clone()));

    // Check persistent values
    let result = db.persistent_values(&SystemTime::now());
    assert_eq!(result.is_ok(), true);
    let mut values = result.ok().unwrap();
    assert_eq!(values.len(), 2);

    let mut map = HashMap::new();
    while let Some(item) = values.pop() {
        map.insert(item.id(), item);
    }
    assert_eq!(map.get(&value2_id).unwrap(), &value2);
    assert_eq!(map.get(&value3_id).unwrap(), &value3);

    // check ids.
    let result = db.value_ids();
    assert_eq!(result.is_ok(), true);
    let mut ids = result.ok().unwrap();
    assert_eq!(ids.len(), 3);
    let mut map: HashMap<Id, Id> = HashMap::new();
    while let Some(item) = ids.pop() {
        map.insert(item.clone(), item);
    }
    assert_eq!(map.get(&value1_id).unwrap(), &value1_id);
    assert_eq!(map.get(&value2_id).unwrap(), &value2_id);
    assert_eq!(map.get(&value3_id).unwrap(), &value3_id);

    // remove all values
    let result = db.remove_value(&value1_id);
    assert_eq!(result.is_ok(), true);
    let result = db.remove_value(&value2_id);
    assert_eq!(result.is_ok(), true);
    let result = db.remove_value(&value3_id);
    assert_eq!(result.is_ok(), true);

    remove_storage(&path);
}

#[test]
#[serial]
fn test_peer() {
    let (mut db, path) = get_storage();

    // create a new peerinfo
    let nodeid = Id::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_port(32222)
        .build();
    let peer_id = peer.id();

    // Check the peerinfo from storage where it should not exist.
    let result = db.peer(&peer_id, &nodeid);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.ok().unwrap().is_some(), false);

    // put peerinfo to storage
    let result = db.put_peer(&peer, Some(true), Some(false));
    assert_eq!(result.is_ok(), true);

    // Recheck the peer from storage where it should
    // be now available.
    let result = db.peer(&peer_id, &nodeid);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().unwrap().is_some(), true);
    assert_eq!(result.ok().unwrap(), Some(peer.clone()));

    // update peer be announced.
    let result = db.update_value_last_announce(&peer_id);
    assert_eq!(result.is_ok(), true);

    // query ids
    let result = db.peer_ids();
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().unwrap().len(), 1);
    assert_eq!(result.as_ref().ok().unwrap()[0], peer_id.clone());

    // remove peer
    let result = db.remove_peer(&peer_id, &nodeid);
    assert_eq!(result.is_ok(), true);

    // Recheck the peer from storage where it should
    // no longer be available.
    let result = db.value(&peer_id);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().unwrap().is_some(), false);

    remove_storage(&path);
}

#[test]
#[serial]
fn test_peers() {
    let (mut db, path) = get_storage();

    // create a new peerinfo
    let nodeid1 = Id::random();
    let peer1 = PeerBuilder::new(&nodeid1)
        .with_port(32222)
        .build();

    let peer1_id = peer1.id();

    // create a new peerinfo
    let nodeid2 = Id::random();
    let origin2 = Id::random();

    let peer2 = PeerBuilder::new(&nodeid2)
        .with_origin(Some(&origin2))
        .with_port(32222)
        .build();
    let peer2_id = peer2.id();

    // create a new peerinfo
    let nodeid3 = Id::random();
    let origin3 = Id::random();

    let peer3 = PeerBuilder::new(&nodeid3)
        .with_origin(Some(&origin3))
        .with_port(32222)
        .with_alternative_url(Some("https://myexample.com"))
        .build();
    let peer3_id = peer3.id();

    // Put them into storage.
    let result = db.put_peer(&peer1, Some(false), Some(false));
    assert_eq!(result.is_ok(), true);
    let result = db.put_peer(&peer2, Some(true), Some(false));
    assert_eq!(result.is_ok(), true);
    let result = db.put_peer(&peer3, Some(true), Some(false));
    assert_eq!(result.is_ok(), true);

    // Check peer1
    let result = db.peer(&peer1_id, &nodeid1);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().is_some(), true);
    assert_eq!(result.ok().unwrap(), Some(peer1.clone()));

    // Check peer2
    let result = db.peer(&peer2_id, &origin2);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().is_some(), true);
    assert_eq!(result.ok().unwrap(), Some(peer2.clone()));

    // Check peer3
    let result = db.peer(&peer3_id, &origin3);
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.as_ref().ok().is_some(), true);
    assert_eq!(result.ok().unwrap(), Some(peer3.clone()));

    // Check persistent peers
    let result = db.persistent_peers(&SystemTime::now());
    assert_eq!(result.is_ok(), true);
    let mut peers = result.ok().unwrap();
    assert_eq!(peers.len(), 2);
    let mut map: HashMap<Id, PeerInfo> = HashMap::new();
    while let Some(item) = peers.pop() {
        map.insert(item.id().clone(), item);
    }
    assert_eq!(map.get(peer2_id).unwrap(), &peer2);
    assert_eq!(map.get(peer3_id).unwrap(), &peer3);

    // check ids.
    let result = db.peer_ids();
    assert_eq!(result.is_ok(), true);
    let mut ids = result.ok().unwrap();
    assert_eq!(ids.len(), 3);
    let mut map: HashMap<Id, Id> = HashMap::new();
    while let Some(item) = ids.pop() {
        map.insert(item.clone(), item);
    }
    assert_eq!(map.get(&peer1_id).unwrap(), peer1_id);
    assert_eq!(map.get(&peer2_id).unwrap(), peer2_id);
    assert_eq!(map.get(&peer3_id).unwrap(), peer3_id);

    // remove all values
    let result = db.remove_peer(&peer1_id, &nodeid1);
    assert_eq!(result.is_ok(), true);
    let result = db.remove_peer(&peer2_id, &origin2);
    assert_eq!(result.is_ok(), true);
    let result = db.remove_peer(&peer3_id, &origin3);
    assert_eq!(result.is_ok(), true);

    remove_storage(&path);
}
