// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{
    fs,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use boson::{
    dht::{Node, NodeOptions},
    did::{DHTRegistry},
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
        .to_hexstr();
    let config_path = path.join("node.yaml");
    let yaml = format!(
        "ipv4: true\nport: 39001\nprivateKey: \"{private_key}\"\ndataDir: {}\ndatabaseUri: jdbc:sqlite:node.db\nlogLevel: \"error\"\n",
        path.display()
    );
    fs::write(&config_path, yaml).unwrap();
    let options = NodeOptions::load(&config_path).unwrap();
    let node = Node::new(options).unwrap();
    (node, path)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
