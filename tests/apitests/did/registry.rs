use std::{
    fs,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use boson::{
    cfg::configuration,
    dht::Node,
    did::{DHTRegistry, Registry},
    CryptoIdentity,
};
use serial_test::serial;

fn create_node() -> (Arc<Node>, std::path::PathBuf) {
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(format!("boson-registry-{suffix}"));
    fs::create_dir_all(&path).unwrap();
    let private_key = boson::signature::KeyPair::random()
        .private_key()
        .to_string();
    let config_path = path.join("node.yaml");
    let yaml = format!(
        "ipv4: true\nport: 0\nprivateKey: \"{private_key}\"\ndataDir: {}\ndatabaseUri: jdbc:sqlite:node.db\nlogLevel: \"error\"\n",
        path.display()
    );
    fs::write(&config_path, yaml).unwrap();
    let config = configuration::Builder::new()
        .load_from(&config_path)
        .unwrap()
        .build()
        .unwrap();
    let node = Node::new(config.build_node_options().unwrap()).unwrap();
    (node, path)
}

#[test]
#[serial]
fn dht_registry_new() {
    let (node, path) = create_node();
    let registry = DHTRegistry::new(node, None);
    let _resolver = registry.resolver();
    drop(registry);
    let _ = fs::remove_dir_all(path);
}

#[tokio::test]
#[serial]
async fn register_rejects_negative_version() {
    let (node, path) = create_node();
    let registry = DHTRegistry::new(node, None);
    let identity = CryptoIdentity::new();
    let card = boson::did::Card::builder(identity.clone()).build().unwrap();

    assert!(registry.register(&identity, &card, -1).await.is_err());
    drop(registry);
    let _ = fs::remove_dir_all(path);
}

#[tokio::test]
#[serial]
async fn register_rejects_identity_mismatch() {
    let (node, path) = create_node();
    let registry = DHTRegistry::new(node, None);
    let identity = CryptoIdentity::new();
    let other = CryptoIdentity::new();
    let card = boson::did::Card::builder(other).build().unwrap();

    assert!(registry.register(&identity, &card, 0).await.is_err());
    drop(registry);
    let _ = fs::remove_dir_all(path);
}

#[tokio::test]
#[serial]
async fn register_stores_card() {
    let (node, path) = create_node();
    node.start().await.unwrap();
    let registry = DHTRegistry::new(node.clone(), None);
    let identity = CryptoIdentity::new();
    let card = boson::did::Card::builder(identity.clone()).build().unwrap();

    registry.register(&identity, &card, 0).await.unwrap();
    node.stop().await.unwrap();
    drop(registry);
    drop(node);
    let _ = fs::remove_dir_all(path);
}
