use std::{
    fs,
    sync::Arc,
    time::Duration,
};
use serial_test::serial;
use boson::{
    signature,
    cryptobox::{Nonce, CryptoBox},
    core::{
        PeerBuilder, Result,
        ImmutableBuilder as ValueBuilder,
    },
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

fn cleanup_path(input: &str) {
    remove_working_path(input);
}

fn create_node(port: u16, path: &str) -> Result<Arc<Node>> {
    let private_key = signature::KeyPair::random().private_key().to_string();
    let config_path = format!("{path}/node.yaml");
    let yaml = format!(
        "ipv4: true\nport: {}\nprivateKey: \"{}\"\ndataDir: {}\ndatabaseUri: {}\nlogLevel: \"debug\"\n",
        port,
        private_key,
        path,
        format!("jdbc:sqlite:node.db"),
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
    let rcs   = tokio::join!(
        node1.start(),
        node2.start()
    );
    for rc in [rcs.0, rcs.1] {
        if let Err(e) = rc {
            panic!("Failed to start node: {e}");
        }
    }

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

    _ = tokio::join!(
        node1.stop(),
        node2.stop()
    );
    cleanup_path(&path1);
    cleanup_path(&path2);
}

#[tokio::test]
#[serial]
async fn test_encryption() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");

    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32223, &path2).unwrap();
    let rcs = tokio::join!(
        node1.start(),
        node2.start()
    );
    for rc in [rcs.0, rcs.1] {
        _ = rc.map_err(|e| assert!(false, "Faild to start node: {e}"))
    }

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

    let _ = tokio::join!(
        node1.stop(),
        node2.stop()
    );
    cleanup_path(&path1);
    cleanup_path(&path2);
}

#[tokio::test]
#[serial]
async fn test_signinto() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32223, &path2).unwrap();

    let (r1, r2) = tokio::join!(
        node1.start(),
        node2.start()
    );
    _ = r1.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = r2.map_err(|e| panic!("Failed to start node2: {e}"));

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

    let _ = tokio::join!(
        node1.stop(),
        node2.stop()
    );
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

    let (r1, r2) = tokio::join!(
        node1.start(),
        node2.start()
    );
    _ = r1.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = r2.map_err(|e| panic!("Failed to start node2: {e}"));

    let data = create_random_bytes(32);
    let mut sig = vec![0u8; signature::Signature::BYTES];
    let result = node2.sign(&data, &mut sig);
    assert_eq!(result.is_ok(), true);

    let result = node2.verify(&data, &sig);
    assert_eq!(result.is_ok(), true);

    let _ = tokio::join!(
        node1.stop(),
        node2.stop()
    );
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

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = rc3.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| panic!("Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to bootstrapping node1 on node3: {e}"));

    tokio::time::sleep(Duration::from_millis(1000)).await;

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
                println!("\x1b[31mfound target {} on node {}\x1b[0m",
                    node2.id(), node1.id());
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
                println!("\x1b[32mfound target {} on node {}\x1b[0m",
                    node3.id(), node2.id());
            });
        }
        _ => {
            assert!(false);
        }
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop()
    );
    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_store_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = rc3.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| panic!("Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to bootstrapping node1 on node3: {e}"));

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

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop()
    );
    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_announce_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = rc3.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| panic!("Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to bootstrapping node1 on node3: {e}"));

    //tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

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

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop()
    );

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_find_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| panic!("Failed to start node1: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to start node2: {e}"));
    _ = rc3.map_err(|e| panic!("Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| panic!("Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| panic!("Failed to bootstrapping node1 on node3: {e}"));
    // tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

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
        Ok(_) => {
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
        Ok(_) => {
            assert!(false);
            panic!("Should have found the value");
        },
        Err(e) => panic!("Find value error: {}", e),
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop()
    );

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_find_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to start node1: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to start node2: {e}"));
    _ = rc3.map_err(|e| assert!(false, "Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node3: {e}"));

    // tokio::time::sleep(Duration::from_millis(3 * 1000)).await;

    assert_eq!(node1.is_running(), true);
    assert_eq!(node2.is_running(), true);
    assert_eq!(node3.is_running(), true);

    let peer = PeerBuilder::new("https://example.com")
        .build()
        .expect("Failed to build peer");

    match node1.announce_peer(&peer, -1, false).await {
        Ok(_) => assert!(true),
        Err(e) => assert!(false, "Announce peer error: {}", e)
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
        Err(e) => assert!(false, "Find peer error: {}", e),
    }
    match result.1 {
        Ok(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].id(), peer.id());
            assert_eq!(v[0].endpoint(), peer.endpoint());
            assert_eq!(v[0].nodeid(), peer.nodeid());
        },
        Err(e) => assert!(false, "Find peer error: {}", e),
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop()
    );

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_get_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to start node1: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to start node2: {e}"));
    _ = rc3.map_err(|e| assert!(false, "Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node3: {e}"));

    //tokio::time::sleep(Duration::from_millis(2 * 1000)).await;

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    let _ = tokio::join!(
        node1.store_value(&value, -1, false),
    );

    let value_id = value.id();
    let result = node1.value(value_id);
    match result {
        Ok(Some(v)) => {
            assert_eq!(v.id(), value_id);
            assert_eq!(v.data(), value.data());
            //assert_eq!(v, value);
        },
        Ok(_) => panic!("Should have found the value"),
        Err(e) => panic!("get value error: {}", e),
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop(),
    );

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_get_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to start node1: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to start node2: {e}"));
    _ = rc3.map_err(|e| assert!(false, "Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node3: {e}"));

    // tokio::time::sleep(Duration::from_millis(2 * 1000)).await;

    let peer = PeerBuilder::new("https://example.com")
        .build()
        .expect("Failed to build peer");

    let _ = tokio::join!(
        node1.announce_peer(&peer, -1, false)
    );

    let peer_id = peer.id().clone();
    let result = node1.peer(peer_id.clone(), peer.fingerprint()).await;
    match result {
        Ok(Some(v)) => {
            assert_eq!(v.id(), &peer_id);
            assert_eq!(v.signature(), peer.signature());
            assert_eq!(v.fingerprint(), peer.fingerprint());
            //assert_eq!(v, peer);
        },
        Ok(_) => panic!("Should have found the peer"),
        Err(e) => panic!("get peer error: {}", e),
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop(),
    );
    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_remove_value() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to start node1: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to start node2: {e}"));
    _ = rc3.map_err(|e| assert!(false, "Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node3: {e}"));

    // tokio::time::sleep(Duration::from_millis(2 * 1000)).await;

    let data = create_random_bytes(32);
    let value = ValueBuilder::new(&data)
        .build()
        .expect("Failed to build immutable value");

    let _ = tokio::join!(
        node1.store_value(&value, -1, false)
    );

    let value_id = value.id();
    let result = node1.remove_value(value_id.clone());
    match result {
        Ok(_) => assert!(true),
        Err(e) => panic!("remove value error: {}", e),
    }

    let result = node1.value(value_id);
    match result {
        Ok(Some(_)) => assert!(false),
        Ok(_) => assert!(true),
        Err(e) => panic!("get value error: {}", e),
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop(),
    );

    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}

#[tokio::test]
#[serial]
async fn test_remove_peer() {
    let path1 = working_path("node1");
    let path2 = working_path("node2");
    let path3 = working_path("node3");
    let node1 = create_node(32222, &path1).unwrap();
    let node2 = create_node(32224, &path2).unwrap();
    let node3 = create_node(32226, &path3).unwrap();

    let (rc1, rc2, rc3) = tokio::join!(
        node1.start(),
        node2.start(),
        node3.start()
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to start node1: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to start node2: {e}"));
    _ = rc3.map_err(|e| assert!(false, "Failed to start node3: {e}"));

    let ni = node1.node_info().v4().expect("No Ipv4 nodeinfo").clone();

    let (rc1, rc2) = tokio::join!(
        node2.bootstrap_one(&ni),
        node3.bootstrap_one(&ni)
    );
    _ = rc1.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node2: {e}"));
    _ = rc2.map_err(|e| assert!(false, "Failed to bootstrapping node1 on node3: {e}"));

    // tokio::time::sleep(Duration::from_secs(2)).await;

    let peer = PeerBuilder::new("https://example.com")
        .build()
        .expect("Failed to build peer");

    let _ = tokio::join!(
        node1.announce_peer(&peer, -1, false)
    );

    let peer_id = peer.id().clone();
    let result = node1.remove_peer(peer_id.clone(), peer.fingerprint()).await;
    match result {
        Ok(_) => assert!(true),
        Err(e) => panic!("remove peer error: {}", e),
    }

    let result = node1.peer(peer_id, peer.fingerprint()).await;
    match result {
        Ok(Some(_)) => assert!(false),
        Ok(_) => assert!(true),
        Err(e) => panic!("get peer error: {}", e),
    }

    let _ = tokio::join!(
        node1.stop(),
        node2.stop(),
        node3.stop(),
    );
    remove_working_path(&path1);
    remove_working_path(&path2);
    remove_working_path(&path3);
}
