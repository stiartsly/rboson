use std::net::SocketAddr;
use tokio::time::Duration;
use tokio::time::sleep;
use serial_test::serial;

use boson::{
    configuration as cfg,
    Id,
    NodeInfo,
    Node,
    ValueBuilder,
    PeerBuilder,
    cryptobox::{Nonce, CryptoBox},
    signature::Signature,
    Identity,
};
use crate::{
    create_random_bytes,
    local_addr,
    working_path,
    remove_working_path,
};

static mut PATH1: Option<String> = None;
static mut PATH2: Option<String> = None;
static mut PATH3: Option<String> = None;

static mut NODE1: Option<Node> = None;
static mut NODE2: Option<Node> = None;
static mut NODE3: Option<Node> = None;

fn setup() {
    unsafe {
        let ip = match local_addr(true) {
            Some(addr) => addr,
            None => panic!("Failed to fetch IP address!!!")
        };

        PATH1 = Some(working_path("node1"));
        PATH2 = Some(working_path("node2"));
        PATH3 = Some(working_path("node3"));

        remove_working_path(PATH1.as_ref().unwrap().as_str());
        remove_working_path(PATH2.as_ref().unwrap().as_str());
        remove_working_path(PATH3.as_ref().unwrap().as_str());

        let ipstr = ip.to_string();
        let cfg1 = cfg::Builder::new()
            .with_listening_port(32222)
            .with_ipv4(&ipstr)
            .with_storage_path(&PATH1.as_ref().unwrap())
            .build()
            .unwrap();

        let cfg2 = cfg::Builder::new()
            .with_listening_port(32224)
            .with_ipv4(&ipstr)
            .with_storage_path(&PATH2.as_ref().unwrap())
            .build()
            .unwrap();

        let cfg3 = cfg::Builder::new()
            .with_listening_port(32226)
            .with_ipv4(&ipstr)
            .with_storage_path(PATH3.as_ref().unwrap())
            .build()
            .unwrap();

        NODE1 = Some(Node::new(&cfg1).unwrap());
        NODE2 = Some(Node::new(&cfg2).unwrap());
        NODE3 = Some(Node::new(&cfg3).unwrap());

        NODE1.as_mut().unwrap().start();
        NODE2.as_mut().unwrap().start();
        NODE3.as_mut().unwrap().start();

        let id = NODE1.as_ref().unwrap().id().clone();
        let p  = NODE1.as_ref().unwrap().port();
        let ni = NodeInfo::new(id, SocketAddr::new(ip, p));

        NODE2.as_mut().unwrap().bootstrap(&ni);
/*
        let id = NODE2.as_ref().unwrap().id().clone();
        let p  = NODE2.as_ref().unwrap().port();
        let ni = NodeInfo::new(id, SocketAddr::new(ip, p));
*/
        NODE3.as_mut().unwrap().bootstrap(&ni);

    }
}

fn teardown() {
    unsafe {
        NODE1.take().unwrap().stop();
        NODE2.take().unwrap().stop();
        NODE3.take().unwrap().stop();

        remove_working_path(PATH1.take().unwrap().as_str());
        remove_working_path(PATH2.take().unwrap().as_str());
        remove_working_path(PATH3.take().unwrap().as_str());
    }
}

#[test]
#[serial]
fn test_encryption_into() {
    setup();
    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        let node2 = NODE2.as_mut().unwrap();

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
    }
    teardown()
}

#[test]
#[serial]
fn test_encryption() {
    setup();
    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        let node2 = NODE2.as_mut().unwrap();

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
    }
    teardown()
}

#[test]
#[serial]
fn test_signinto() {
    setup();
    unsafe {
        let node1 = NODE1.as_mut().unwrap();

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
    }
    teardown()
}

#[test]
#[serial]
fn test_sign() {
    setup();
    unsafe {
        let node2 = NODE2.as_mut().unwrap();

        let data = create_random_bytes(32);
        let mut sig = vec![0u8; Signature::BYTES];
        let result = node2.sign(&data, &mut sig);
        assert_eq!(result.is_ok(), true);

        let result = node2.verify(&data, &sig);
        assert_eq!(result.is_ok(), true);
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_find_node() {
    setup();
    sleep(Duration::from_millis(3*1000)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        let node2 = NODE2.as_mut().unwrap();
        let node3 = NODE3.as_mut().unwrap();

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_store_value() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node = NODE1.as_mut().unwrap();
        assert_eq!(node.is_running(), true);

        let data = create_random_bytes(32);
        let value = ValueBuilder::new(&data)
            .build()
            .expect("Failed to build immutable value");

        match node.store_value(&value, None).await {
            Ok(_) => assert!(true),
            Err(_) => panic!("testcase failed")
        }
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_announce_peer() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node = NODE1.as_mut().unwrap();
        assert_eq!(node.is_running(), true);

        let nodeid = Id::random();
        let peer = PeerBuilder::new(&nodeid)
            .with_port(65534)
            .with_alternative_url(Some("http://announce.com"))
            .build();

        match node.announce_peer(&peer, None).await {
            Ok(_) => assert!(true),
            Err(_) => panic!("testcase failed")
        }
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_find_value() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        let node2 = NODE2.as_mut().unwrap();
        let node3 = NODE3.as_mut().unwrap();

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_find_peer() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        let node2 = NODE2.as_mut().unwrap();
        let node3 = NODE3.as_mut().unwrap();

        assert_eq!(node1.is_running(), true);
        assert_eq!(node2.is_running(), true);

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_get_value() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();

        assert_eq!(node1.is_running(), true);

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_get_peer() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        assert_eq!(node1.is_running(), true);

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_remove_value() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();

        assert_eq!(node1.is_running(), true);

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_remove_peer() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_get_value_ids() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        assert_eq!(node1.is_running(), true);

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
    }
    teardown()
}

#[tokio::test]
#[serial]
async fn test_get_peer_ids() {
    setup();
    sleep(Duration::from_secs(2)).await;

    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        assert_eq!(node1.is_running(), true);

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
    }
    teardown()
}
