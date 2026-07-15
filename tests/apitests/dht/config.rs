use std::env;
use log::LevelFilter;
use boson::{
    dht::{NodeConfig, NodeConfiguration},
    signature::{KeyPair, PrivateKey},
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config_from_memory() {
        let private_key = KeyPair::random().private_key().to_string();
        let yaml = format!(
            "ipv4: true\nport: 39001\nprivateKey: \"{private_key}\"\ndataDir: tests-data\ndatabaseUri: sqlite://node.db\nbootstraps:\n  - - 2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k\n    - 203.0.113.5\n    - 39011\nlogLevel: debug\nlogFile: node.log\nenableDeveloperMode: true\n"
        );

        let cfg = NodeConfiguration::from(&yaml).unwrap();

        assert!(cfg.host4().is_some());
        assert_eq!(cfg.host6(), None);
        assert_eq!(cfg.port(), 39001);
        assert_eq!(cfg.private_key(), &PrivateKey::try_from(private_key.as_str()).unwrap());
        assert_eq!(cfg.data_dir(), "tests-data");
        assert_eq!(cfg.database_uri(), "sqlite://node.db");
        assert_eq!(cfg.bootstrap_nodes().len(), 1);
        assert_eq!(cfg.bootstrap_nodes()[0].host(), "203.0.113.5");
        assert_eq!(cfg.bootstrap_nodes()[0].port(), 39011);
        assert_eq!(cfg.log_level(), LevelFilter::Debug);
        assert_eq!(cfg.log_file(), Some("node.log"));
        assert!(cfg.enable_devp());
    }

    #[test]
    fn test_node_config_from_yaml_file() {
        let path = match std::fs::metadata("tests.yaml") {
            Ok(_) => "tests.yaml",
            Err(_) => "tests/apitests/dht/tests.yaml",
        };
        unsafe {
            env::set_var(
                "NODE_PUBLIC_KEY",
                "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ",
            );
        }
        let cfg = NodeConfiguration::load(&path).unwrap();

        assert!(cfg.host4().is_some());
        assert_eq!(cfg.host6(), None);
        assert_eq!(cfg.port(), 39001);
        assert_eq!(
            cfg.private_key(),
            &PrivateKey::try_from("0xa3218958b88d86dead1a58b439a22c161e0573022738b570210b123dc0b046faec6f3cd4ed1e6801ebf33fd60c07cf9924ef01d829f3f5af7377f054bff31501").unwrap()
        );
        assert_eq!(cfg.data_dir(), ".");
        assert_eq!(cfg.database_uri(), "storage.db");
        assert_eq!(cfg.bootstrap_nodes().len(), 2);
        assert_eq!(cfg.bootstrap_nodes()[0].host(), "203.0.113.5");
        assert_eq!(cfg.bootstrap_nodes()[0].port(), 39001);
        assert_eq!(cfg.bootstrap_nodes()[1].host(), "198.51.100.8");
        assert_eq!(cfg.bootstrap_nodes()[1].port(), 39001);
        assert_eq!(cfg.log_level(), LevelFilter::Info);
        assert_eq!(cfg.log_file(), None);
        assert!(!cfg.enable_devp());
    }
}
