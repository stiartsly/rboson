use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    signature::KeyPair,
    dht::{Node, NodeOptions},
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn test_concurrent_start_not_allowed() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "boson-node-lifecycle-{}-{unique}",
            std::process::id(),
        ));
        let sk = KeyPair::random().to_private_key();
        let options = NodeOptions::new(sk)
            .with_host4("127.0.0.1")
            .with_port(39111)
            .with_data_dir(data_dir.to_str().unwrap());
        let node = Node::new(options).unwrap();

        let (first, second) = tokio::join!(node.start(), node.start());
        assert_ne!(first.is_ok(), second.is_ok());
        assert!(node.is_running());

        node.stop().await.unwrap();
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn node_info_requires_running_node() {
        let data_dir = std::env::temp_dir().join(format!(
            "boson-node-info-{}",
            std::process::id(),
        ));
        let sk = KeyPair::random().to_private_key();
        let options = NodeOptions::new(sk)
            .with_host4("127.0.0.1")
            .with_port(39112)
            .with_data_dir(data_dir.to_str().unwrap());
        let node = Node::new(options).unwrap();

        assert!(node.node_info().is_err());

        fs::remove_dir_all(data_dir).unwrap();
    }
}
