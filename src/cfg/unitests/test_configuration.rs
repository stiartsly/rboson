use std::{env, fs};

use log::LevelFilter;

use crate::{
    Id,
    signature::{KeyPair, PrivateKey},
    cfg::{
        NodeConfig,
        ActiveProxyConfig,
        config::DEFAULT_DHT_PORT,
        configuration::Builder,
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let random_suffix = format!("{:016x}", rand::random::<u64>());
        let path = env::temp_dir().join(format!("{prefix}-{unique}-{random_suffix}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_nodeconfig() {
        let keypair = KeyPair::random();

        let cfg = Builder::new()
            .with_host4("127.0.0.1")
            .with_port(39001)
            .with_private_key(keypair.private_key().clone())
            .with_data_dir("./cfg-tests")
            .with_database_uri("sqlite://node.db")
            .with_log_level(LevelFilter::Debug)
            .with_log_file("node.log")
            .with_log_console(false)
            .with_devmode(true)
            .build()
            .unwrap();

        let node_cfg: &dyn NodeConfig = &cfg;
        assert_eq!(node_cfg.host4(), Some("127.0.0.1"));
        assert_eq!(node_cfg.host6(), None);
        assert_eq!(node_cfg.port(), 39001);
        assert_eq!(node_cfg.private_key(), keypair.private_key());
        assert_eq!(node_cfg.data_dir(), "./cfg-tests");
        assert_eq!(node_cfg.database_uri(), "sqlite://node.db");
        assert_eq!(node_cfg.log_level(), LevelFilter::Debug);
        assert_eq!(node_cfg.log_file(), Some("node.log"));
        assert_eq!(node_cfg.log_console(), false);
        assert_eq!(node_cfg.enable_devp(), true);
        assert!(node_cfg.bootstrap_nodes().is_empty());
    }

    #[test]
    fn test_activeproxyconfig() {
        let keypair = KeyPair::random();
        let proxy_keypair = KeyPair::random();
        let server_peerid = Id::random();

        let cfg = Builder::new()
            .with_host4("127.0.0.1")
            .with_private_key(keypair.private_key().clone())
            .with_database_uri("sqlite://node.db")
            .with_active_proxy_server_peerid(server_peerid)
            .with_active_proxy_peer_private_key(proxy_keypair.private_key().clone())
            .with_active_proxy_upstream_host("upstream.example")
            .with_active_proxy_upstream_port(18080)
            .build()
            .unwrap();

        let ap_cfg: &dyn ActiveProxyConfig = &cfg;
        assert_eq!(ap_cfg.server_peerid().unwrap().to_string(), server_peerid.to_string());
        assert_eq!(ap_cfg.peer_private_key().unwrap(), proxy_keypair.private_key());
        assert_eq!(ap_cfg.upstream_host(), Some("upstream.example"));
        assert_eq!(ap_cfg.upstream_port(), Some(18080));
    }

    #[test]
    fn test_yaml_configuration() {
        let keypair = KeyPair::random();
        let private_key = keypair.private_key().to_string();
        let temp_dir = make_temp_dir("cfg-yaml");
        let path = temp_dir.join("node.yaml");

        let yaml = format!(
            "ipv4: true\nport: 39011\nprivateKey: \"{private_key}\"\ndataDir: ./yaml-data\ndatabaseUri: sqlite://node.db\nbootstraps:\n  - - 2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k\n    - 203.0.113.5\n    - 39001\nlogLevel: debug\nlogFile: node.log\nlogConsole: false\nenableDeveloperMode: true\nactiveproxy:\n  serverPeerId: 5vVM1nrCwFh3QqAgbvF3bRgYQL5a2vpFjngwxkiS8Ja6\n  upstreamHost: 127.0.0.1\n  upstreamPort: 8080\n"
        );

        fs::write(&path, yaml).unwrap();
        let cfg = Builder::new().load(&path).unwrap().build().unwrap();

        assert!(cfg.host4().is_some());
        assert_eq!(cfg.port(), 39011);
        assert_eq!(cfg.private_key(), &PrivateKey::try_from(private_key.as_str()).unwrap());
        assert_eq!(cfg.data_dir(), "./yaml-data");
        assert_eq!(cfg.database_uri(), "sqlite://node.db");
        assert_eq!(cfg.log_level(), LevelFilter::Debug);
        assert_eq!(cfg.log_file(), Some("node.log"));
        assert_eq!(cfg.log_console(), false);
        assert_eq!(cfg.enable_devp(), true);
        assert_eq!(cfg.server_peerid().unwrap().to_string(), "5vVM1nrCwFh3QqAgbvF3bRgYQL5a2vpFjngwxkiS8Ja6");
        assert_eq!(cfg.upstream_host(), Some("127.0.0.1"));
        assert_eq!(cfg.upstream_port(), Some(8080));

        fs::remove_file(&path).unwrap();
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_load_configuration() {
        let keypair = KeyPair::random();
        let private_key = keypair.private_key().to_string();
        let temp_dir = make_temp_dir("cfg-combined");
        let path = temp_dir.join("node.yaml");

        let yaml = format!(
            "ipv4: true\nport: 39001\nprivateKey: \"{private_key}\"\ndataDir: ./yaml-data\ndatabaseUri: sqlite://node.db\nlogConsole: false\n"
        );
        fs::write(&path, yaml).unwrap();

        let proxy_sk = KeyPair::random().private_key().clone();
        let cfg = Builder::new()
            .load(&path).unwrap()
            .with_port(40111)
            .with_data_dir("./override-data")
            .with_log_console(true)
            .with_active_proxy_server_peerid(Id::random())
            .with_active_proxy_peer_private_key(proxy_sk.clone())
            .with_active_proxy_upstream_host("override.host")
            .with_active_proxy_upstream_port(18081)
            .build()
            .unwrap();

        assert_eq!(cfg.port(), 40111);
        assert_eq!(cfg.data_dir(), "./override-data");
        assert_eq!(cfg.database_uri(), "sqlite://node.db");
        assert_eq!(cfg.log_console(), true);
        assert_eq!(cfg.peer_private_key(), Some(&proxy_sk));
        assert_eq!(cfg.upstream_host(), Some("override.host"));
        assert_eq!(cfg.upstream_port(), Some(18081));

        fs::remove_file(&path).unwrap();
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_builder_missing_optional() {
        let keypair = KeyPair::random();

        let cfg = Builder::new()
            .with_host4("127.0.0.1")
            .with_private_key(keypair.private_key().clone())
            .with_database_uri("sqlite://node.db")
            .build()
            .unwrap();

        assert_eq!(cfg.port(), DEFAULT_DHT_PORT);
        assert_eq!(cfg.data_dir(), ".");
        assert_eq!(cfg.log_level(), LevelFilter::Info);
        assert_eq!(cfg.log_file(), None);
        assert_eq!(cfg.server_peerid(), None);
        assert_eq!(cfg.upstream_host(), None);
        assert_eq!(cfg.upstream_port(), None);
    }
}
