use std::net::SocketAddr;
use tokio::time::Duration;
use tokio::time::sleep;
use serial_test::serial;
use once_cell::sync::Lazy;

use boson::{
    configuration as cfg,
    Id,
    NodeInfo,
    ValueBuilder,
    PeerBuilder,
    cryptobox::{Nonce, CryptoBox},
    signature::Signature,
    Identity,
    core::Result,
    dht::Node,
};
use crate::{
    create_random_bytes,
    local_addr,
    working_path,
    remove_working_path,
};

static PATH1: Lazy<String> = Lazy::new(|| working_path("node1"));
static PATH2: Lazy<String> = Lazy::new(|| working_path("node2"));
static PATH3: Lazy<String> = Lazy::new(|| working_path("node3"));

fn create_node(port: u16, path: &str) -> Result<Node> {
    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ipstr = ip.to_string();
    let cfg = cfg::Builder::new()
        .with_port(port)
        .with_ipv4(&ipstr)
        .with_data_dir(path)
        .build()
        .unwrap();

    Ok(Node::new(&cfg).unwrap())
}

#[test]
#[serial]
fn test_encryption_into() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    let plain = create_random_bytes(32);
    let result = node1.encrypt_into(node2.id(), &plain);
    let cipher = match result {
        Ok(cipher) => {
            assert!(true);
            assert_eq!(cipher.len(), plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES);
            cipher
        },
        Err(_) => {
            assert!(false);
            panic!("testcase failed");
        }
    };

    let result = node2.decrypt_into(node1.id(), &cipher);
    match result {
        Ok(decrypted) => {
            assert!(true);
            assert_eq!(decrypted.len() +  CryptoBox::MAC_BYTES + Nonce::BYTES, cipher.len());
            assert_eq!(plain.len(), decrypted.len());
            assert_eq!(plain, decrypted);
        }
        Err(_) => {
            assert!(false);
            panic!("testcase failed");
        }
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[test]
#[serial]
fn test_encryption() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    let plain = create_random_bytes(32);
    let mut cipher = vec![0u8; 1024];
    let result = node1.encrypt(node2.id(), &plain, &mut cipher);
    let cipher_len = match result {
        Ok(cipher_len) => {
            assert!(true);
            assert_eq!(cipher_len, plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES);
            cipher_len
        },
        Err(_) => {
            assert!(false);
            panic!("testcase failed");
        }
    };

    let mut decrypted = vec![0u8; 1024];
    let result = node2.decrypt(node1.id(), &cipher[..cipher_len], &mut decrypted);
    match result {
        Ok(decrypted_len) => {
            assert!(true);
            assert_eq!(decrypted_len +  CryptoBox::MAC_BYTES + Nonce::BYTES, cipher_len);
            assert_eq!(decrypted_len, plain.len());
            assert_eq!(plain, decrypted[..decrypted_len]);
        }
        Err(_) => {
            assert!(false);
            panic!("testcase failed");
        }
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[test]
#[serial]
fn test_signinto() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    let data = create_random_bytes(32);
    let result = node1.sign_into(&data);
    let sig = match result {
        Ok(sig) => {
            assert!(true);
            assert_eq!(sig.len(), Signature::BYTES);
            sig
        },
        Err(_) => {
            assert!(false);
            panic!("testcase failed");
        }
    };

    let result = node1.verify(&data, &sig);
    match result {
        Ok(_) => assert!(true),
        Err(_) => {
            assert!(false);
            panic!("testcase failed");
        }
    };

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}


#[test]
#[serial]
fn test_sign() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    let data = create_random_bytes(32);
    let mut sig = vec![0u8; Signature::BYTES];
    let result = node2.sign(&data, &mut sig);
    assert_eq!(result.is_ok(), true);

    let result = node2.verify(&data, &sig);
    assert_eq!(result.is_ok(), true);

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_find_node() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(3*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let result = tokio::join!(
        node1.find_node(node2.id(), None),
        node2.find_node(node3.id(), None)
    );

    match result.0 {
        Ok(found) => {
            assert!(found.v4().is_some());
            assert!(found.v6().is_none());

            found.v4().map(|ni| {
                assert!(ni.id() == node2.id());
                println!("found target {} on node {}", node2.id(), node1.id());
            });
        }
        _ => {
            assert!(false);
        }
    }

    match result.1 {
        Ok(found) => {
            assert!(found.v4().is_some());
            assert!(found.v6().is_none());

            found.v4().map(|ni| {
                assert!(ni.id() == node3.id());
                println!("found target {} on node {}", node3.id(), node2.id());
            });
        }
        _ => {
            assert!(false);
        }
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_store_value() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(3*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    match node1.store_value(&value, None).await {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_announce_peer() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(3*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let nodeid = Id::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_port(65534)
        .with_alternative_url(Some("http://announce.com"))
        .build();

    match node1.announce_peer(&peer, None).await {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_find_value() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(3*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    match node1.store_value(&value, None).await {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    let value_id = value.id();
    let result = tokio::join!(
        node2.find_value(&value_id, None),
        node3.find_value(&value_id, None)
    );
    match result.0 {
        Ok(Some(v)) => {
            assert_eq!(value.id(), v.id());
            assert_eq!(v.is_mutable(), false);
            assert_eq!(value.data(), v.data());
        },
        Ok(None) => {
            assert!(false);
            panic!("Should have found the value");
        },
        Err(e) => panic!("Find value error: {}", e),
    }
    match result.1 {
        Ok(Some(v)) => {
            assert_eq!(value.id(), v.id());
            assert_eq!(v.is_mutable(), false);
            assert_eq!(value.data(), v.data());
        },
        Ok(None) => {
            assert!(false);
            panic!("Should have found the value");
        },
        Err(e) => panic!("Find value error: {}", e),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_find_peer() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(3*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer = PeerBuilder::new(node1.id())
        .with_port(65534)
        .with_alternative_url(Some("http://example.com"))
        .build();

    match node1.announce_peer(&peer, None).await {
        Ok(_) => assert!(true),
        Err(e) => panic!("Announce value error: {}", e)
    }

    let peer_id = peer.id();
    let result = tokio::join!(
        node2.find_peer(peer_id, None, None),
        node3.find_peer(peer_id, None, None)
    );

    match result.0 {
        Ok(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].id(), peer.id());
            assert_eq!(v[0].nodeid(), node1.id());
            assert_eq!(v[0].origin(), node1.id());
            assert_eq!(v[0].is_delegated(), false);
        },
        Err(e) => panic!("Find peer error: {}", e),
    }
    match result.1 {
        Ok(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].id(), peer.id());
            assert_eq!(v[0].nodeid(), node1.id());
            assert_eq!(v[0].origin(), node1.id());
            assert_eq!(v[0].is_delegated(), false);
        },
        Err(e) => panic!("Find peer error: {}", e),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_get_value() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(2*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    match node1.store_value(&value, None).await {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    let value_id = value.id();
    let result = node1.value(&value_id).await;
    match result {
        Ok(Some(v)) => {
            assert_eq!(v.id(), value_id);
            assert_eq!(v, value);
        },
        Ok(None) => {
            assert!(false);
            panic!("Should have found the value");
        },
        Err(e) => panic!("get value error: {}", e),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_get_peer() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(2*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer = PeerBuilder::new(node1.id())
        .with_port(65534)
        .with_alternative_url(Some("http://example.com"))
        .build();

    match node1.announce_peer(&peer, None).await {
        Ok(_) => assert!(true),
        Err(e) => panic!("Announce value error: {}", e)
    }

    let peer_id = peer.id();
    let result = node1.peer(peer_id).await;
    match result {
        Ok(Some(v)) => {
            assert_eq!(v.id(), peer_id);
            assert_eq!(v, peer);
        },
        Ok(None) => {
            assert!(false);
            panic!("Should have found the peer");
        },
        Err(e) => panic!("get peer error: {}", e),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_remove_value() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_millis(2*1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    match node1.store_value(&value, None).await {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    let value_id = value.id();
    let result = node1.remove_value(&value_id).await;
    match result {
        Ok(_) => assert!(true),
        Err(e) => panic!("remove value error: {}", e),
    }

    let result = node1.value(&value_id).await;
    match result {
        Ok(Some(_)) => assert!(false),
        Ok(None) => assert!(true),
        Err(e) => panic!("get value error: {}", e),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_remove_peer() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_secs(2)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer = PeerBuilder::new(node1.id())
        .with_port(65534)
        .with_alternative_url(Some("http://example.com"))
        .build();

    match node1.announce_peer(&peer, None).await {
        Ok(_) => assert!(true),
        Err(e) => panic!("Announce value error: {}", e)
    }

    let peer_id = peer.id();
    let result = node1.remove_peer(&peer_id).await;
    match result {
        Ok(_) => assert!(true),
        Err(e) => panic!("remove peer error: {}", e),
    }

    let result = node1.peer(&peer_id).await;
    match result {
        Ok(Some(_)) => assert!(false),
        Ok(None) => assert!(true),
        Err(e) => panic!("get peer error: {}", e),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_get_value_ids() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_secs(2)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data1 = create_random_bytes(32);
    let value1 = ValueBuilder::new(&data1)
        .build()
        .unwrap();

    let data2 = create_random_bytes(32);
    let value2 = ValueBuilder::new(&data2)
        .build()
        .unwrap();

    let result = tokio::join!(
        node1.store_value(&value1, None),
        node1.store_value(&value2, None)
    );

    match result.0 {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }
    match result.1 {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    let result = node1.value_ids().await;
    match result {
        Ok(ids) => {
            assert_eq!(ids.len(), 2);
            assert_ne!(ids[0], ids[1]);
            assert_eq!(ids[0].clone() == value1.id() || ids[1].clone() == value1.id(), true);
            assert_eq!(ids[0].clone() == value2.id() || ids[1].clone() == value2.id(), true);
        },
        Err(_) => panic!("testcase failed"),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}

#[tokio::test]
#[serial]
async fn test_get_peer_ids() {
    let node1 = create_node(32222, &PATH1).unwrap();
    let node2 = create_node(32224, &PATH2).unwrap();
    let node3 = create_node(32226, &PATH3).unwrap();

    node1.start();
    node2.start();
    node3.start();

    let ip = match local_addr(true) {
        Some(addr) => addr,
        None => panic!("Failed to fetch IP address!!!")
    };

    let ni = NodeInfo::new(node1.id().clone(), SocketAddr::new(ip, node1.port()));
    node2.bootstrap(&ni);
    node3.bootstrap(&ni);

    sleep(Duration::from_secs(2)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer1 = PeerBuilder::new(node1.id())
        .with_port(65534)
        .with_alternative_url(Some("http://example1.com"))
        .build();

    let peer2 = PeerBuilder::new(node1.id())
        .with_port(65535)
        .with_alternative_url(Some("http://example2.com"))
        .build();

    let result = tokio::join!(
        node1.announce_peer(&peer1, None),
        node1.announce_peer(&peer2, None)
    );

    match result.0 {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }
    match result.1 {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    let result = node1.peer_ids().await;
    match result {
        Ok(ids) => {
            assert_eq!(ids.len(), 2);
            assert_ne!(ids[0], ids[1]);
            assert_eq!(ids[0] == peer1.id().clone() || ids[1] == peer1.id().clone(), true);
            assert_eq!(ids[0] == peer2.id().clone() || ids[1] == peer2.id().clone(), true);
        },
        Err(_) => panic!("testcase failed"),
    }

    node1.stop();
    node2.stop();
    node3.stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
    remove_working_path(&PATH3);
}
