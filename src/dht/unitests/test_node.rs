use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    dht::{
        node::Node,
        node_options::NodeOptions,
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn concurrent_start_allows_only_one_caller() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "boson-node-lifecycle-{}-{unique}",
            std::process::id(),
        ));
        let options = NodeOptions::builder()
            .with_host4("127.0.0.1")
            .with_port(0)
            .with_data_dir(data_dir.to_str().unwrap())
            .with_database_uri("jdbc:sqlite:node.db")
            .build()
            .unwrap();
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
        let options = NodeOptions::builder()
            .with_host4("127.0.0.1")
            .with_data_dir(data_dir.to_str().unwrap())
            .with_database_uri("jdbc:sqlite:node.db")
            .build()
            .unwrap();
        let node = Node::new(options).unwrap();

        assert!(node.node_info().is_err());

        fs::remove_dir_all(data_dir).unwrap();
    }

    #[tokio::test]
    async fn find_peer_rejects_counts_larger_than_i32_max() {
        let data_dir = std::env::temp_dir().join(format!(
            "boson-node-find-peer-{}",
            std::process::id(),
        ));
        let options = NodeOptions::builder()
            .with_host4("127.0.0.1")
            .with_data_dir(data_dir.to_str().unwrap())
            .with_database_uri("jdbc:sqlite:node.db")
            .build()
            .unwrap();
        let node = Node::new(options).unwrap();

        assert!(node
            .find_peer(node.id(), 0, i32::MAX as usize + 1, None)
            .await
            .is_err());

        fs::remove_dir_all(data_dir).unwrap();
    }
}
