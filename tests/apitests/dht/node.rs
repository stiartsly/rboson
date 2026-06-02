use std::{
    time::Duration,
    fs,
    sync::Arc,
};
use serial_test::serial;
use boson::{
    cryptobox::{Nonce, CryptoBox},
    core::{PeerBuilder, Result, ValueBuilder},
    signature,
    dht::{
        NodeConfiguration,
        Node,
    },
};
use crate::{
    create_random_bytes,
    remove_working_path,
};

fn working_path(input: &str) -> String {
    let random_suffix = format!("{:016x}", rand::random::<u64>());

    let path = std::env::current_dir().unwrap().join(format!("{input}-{random_suffix}"));
    if !std::fs::metadata(&path).is_ok() {
        match std::fs::create_dir(&path) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create directory: {}", e);
            }
        }
    }
    path.display().to_string()
}

fn create_node(port: u16, path: &str) -> Result<Arc<Node>> {
    let private_key = signature::KeyPair::random().private_key().to_string();
    let config_path = format!("{path}/node.yaml");
    let yaml = format!(
        "ipv4: true\nport: {}\nprivateKey: \"{}\"\ndataDir: {}\nlogLevel: \"debug\"\n",
        port,
        private_key,
        path,
    );

    fs::write(&config_path, yaml)?;
    let cfg = NodeConfiguration::load(&config_path).unwrap();

    Ok(Node::new(Box::new(cfg))?)
}

#[tokio::test]
#[serial]
async fn test_encryption_into() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32223, &path2).unwrap();

    _ = node1.start().await;
    _ = node2.start().await;

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

    _ = node1.stop().await;
    _ = node2.stop().await;
    remove_working_path(&path1);
    remove_working_path(&path2);
}

#[tokio::test]
#[serial]
async fn test_encryption() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32223, &path2).unwrap();

    _ = node1.start().await;
    _ = node2.start().await;

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

    _ = node1.stop().await;
    _ = node2.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
}

#[tokio::test]
#[serial]
async fn test_signinto() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32223, &path2).unwrap();

    _ = node1.start().await;
    _ = node2.start().await;

    let data = create_random_bytes(32);
    let result = node1.sign_into(&data);
    let sig = match result {
        Ok(sig) => {
            assert!(true);
            assert_eq!(sig.len(), signature::Signature::BYTES);
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

    _ = node1.stop().await;
    _ = node2.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
}

#[tokio::test]
#[serial]
async fn test_sign() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32223, &path2).unwrap();

    _ = node1.start().await;
    _ = node2.start().await;

    let data = create_random_bytes(32);
    let mut sig = vec![0u8; signature::Signature::BYTES];
    let result = node2.sign(&data, &mut sig);
    assert_eq!(result.is_ok(), true);

    let result = node2.verify(&data, &sig);
    assert_eq!(result.is_ok(), true);

    _ = node1.stop().await;
    _ = node2.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
}

#[tokio::test]
#[serial]
async fn test_find_node() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(3*1000)).await;

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

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_store_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    match node1.store_value(&value, -1, false).await {
        Ok(_) => assert!(true),
        Err(e) => panic!("store value error: {}", e)
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_announce_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer = PeerBuilder::new("https://announce.com")
        .build()
        .expect("Failed to build peer");

    match node1.announce_peer(&peer, -1, false).await {
        Ok(_) => assert!(true),
        Err(e) => panic!("announce peer error: {}", e)
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_find_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    match node1.store_value(&value, -1, false).await {
        Ok(_) => assert!(true),
        Err(_) => panic!("testcase failed")
    }

    let value_id = value.id();
    let result = tokio::join!(
        node2.find_value(&value_id, -1, None),
        node3.find_value(&value_id, -1, None)
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

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_find_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer = PeerBuilder::new("https://example.com")
        .build()
        .expect("Failed to build peer");

    match node1.announce_peer(&peer, -1, false).await {
        Ok(_) => assert!(true),
        Err(e) => panic!("Announce peer error: {}", e)
    }

    let peer_id = peer.id().clone();
    let result = tokio::join!(
        node2.find_peer(&peer_id, -1, 1, None),
        node3.find_peer(&peer_id, -1, 1, None)
    );

    match result.0 {
        Ok(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].id(), peer.id());
            assert_eq!(v[0].endpoint(), peer.endpoint());
            assert_eq!(v[0].nodeid(), peer.nodeid());
        },
        Err(e) => panic!("Find peer error: {}", e),
    }
    match result.1 {
        Ok(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].id(), peer.id());
            assert_eq!(v[0].endpoint(), peer.endpoint());
            assert_eq!(v[0].nodeid(), peer.nodeid());
        },
        Err(e) => panic!("Find peer error: {}", e),
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_get_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(2 * 1000)).await;

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    _ = node1.store_value(&value, -1, false).await.unwrap();

    let value_id = value.id();
    let result = node1.value(value_id);
    match result {
        Ok(Some(v)) => {
            assert_eq!(v.id(), value_id);
            assert_eq!(v, value);
        },
        Ok(None) => panic!("Should have found the value"),
        Err(e) => panic!("get value error: {}", e),
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_get_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(2 * 1000)).await;

    let peer = PeerBuilder::new("https://example.com")
        .build()
        .expect("Failed to build peer");

    _ = node1.announce_peer(&peer, -1, false).await.unwrap();

    let peer_id = peer.id().clone();
    let result = node1.peer(peer_id.clone(), peer.fingerprint()).await;
    match result {
        Ok(Some(v)) => {
            assert_eq!(v.id(), &peer_id);
            assert_eq!(v, peer);
        },
        Ok(None) => panic!("Should have found the peer"),
        Err(e) => panic!("get peer error: {}", e),
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_remove_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_millis(2 * 1000)).await;

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    _ = node1.store_value(&value, -1, false).await.unwrap();

    let value_id = value.id();
    let result = node1.remove_value(value_id.clone());
    match result {
        Ok(_) => assert!(true),
        Err(e) => panic!("remove value error: {}", e),
    }

    let result = node1.value(value_id);
    match result {
        Ok(Some(_)) => assert!(false),
        Ok(None) => assert!(true),
        Err(e) => panic!("get value error: {}", e),
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[ignore]
#[tokio::test]
#[serial]
async fn test_remove_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    _ = node1.start().await.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = node2.start().await.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = node3.start().await.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();
    _ = node2.bootstrap_one(&ni).await;
    _ = node3.bootstrap_one(&ni).await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let peer = PeerBuilder::new("https://example.com")
        .build()
        .expect("Failed to build peer");

    _ = node1.announce_peer(&peer, -1, false).await.unwrap();

    let peer_id = peer.id().clone();
    let result = node1.remove_peer(peer_id.clone(), peer.fingerprint()).await;
    match result {
        Ok(_) => assert!(true),
        Err(e) => panic!("remove peer error: {}", e),
    }

    let result = node1.peer(peer_id, peer.fingerprint()).await;
    match result {
        Ok(Some(_)) => assert!(false),
        Ok(None) => assert!(true),
        Err(e) => panic!("get peer error: {}", e),
    }

    _ = node1.stop().await;
    _ = node2.stop().await;
    _ = node3.stop().await;

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}
