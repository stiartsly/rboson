use std::{env, fs};
use log::LevelFilter;

use crate::{
    signature::{KeyPair, PrivateKey},
};
use crate::dht::{
    node_config::NodeConfig,
    configuration::Builder,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_read_config() {
        let keypair = KeyPair::random();
        let private_key = keypair.private_key().to_string();
        let host4 = crate::local_addr(true).unwrap().to_string();
        let data_dir = "./tmp_data";
        let database_uri = "storage.db";
        let yaml = format!(
            "ipv4: true\nport: 39001\nprivateKey: \"{private_key}\"\ndataDir: {data_dir}\ndatabaseUri: {database_uri}\nbootstraps:\n  - - 2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k\n    - 203.0.113.5\n    - 39001\nlogLevel: debug\nlogFile: node.log\nenableDeveloperMode: true\n"
        );

        let cfg = Builder::new().from(&yaml).unwrap().build().unwrap();

        assert_eq!(cfg.host4(), Some(host4.as_str()));
        assert_eq!(cfg.host6(), None);
        assert_eq!(cfg.port(), 39001);
        assert_eq!(cfg.private_key(), &PrivateKey::try_from(private_key.as_str()).unwrap());
        assert_eq!(cfg.data_dir(), data_dir);
        assert_eq!(cfg.database_uri(), database_uri);
        assert_eq!(cfg.bootstrap_nodes().len(), 1);
        assert_eq!(cfg.bootstrap_nodes()[0].host(), "203.0.113.5");
        assert_eq!(cfg.bootstrap_nodes()[0].port(), 39001);
        assert_eq!(cfg.log_level(), LevelFilter::Debug);
        assert_eq!(cfg.log_file(), Some("node.log"));
        assert_eq!(cfg.log_console(), false);
        assert_eq!(cfg.enable_devp(), true);
    }

    #[test]
    fn test_load_config() {
        let keypair = KeyPair::random();
        let private_key = keypair.private_key().to_string();
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let random_suffix = format!("{:016x}", rand::random::<u64>());
        let temp_dir = env::temp_dir().join(format!("tmp-{unique}-{random_suffix}"));
        fs::create_dir_all(&temp_dir).unwrap();

        let home_dir = temp_dir.join("home");
        fs::create_dir_all(&home_dir).unwrap();

        unsafe {
            env::set_var("HOME", &home_dir);
            env::set_var("NODE_PRIVATE_KEY", &private_key);
        }

        let path = temp_dir.join("node.yaml");
        fs::write(
            &path,
            "ipv4: true\nprivateKey: ${NODE_PRIVATE_KEY}\ndataDir: ~/node-data\ndatabaseUri: sqlite://node.db\n",
        ).unwrap();

        let cfg = Builder::new().load(&path).unwrap().build().unwrap();

        assert_eq!(cfg.host4().is_some(), true);
        assert_eq!(cfg.private_key(), &PrivateKey::try_from(private_key.as_str()).unwrap());
        assert_eq!(cfg.data_dir(), home_dir.join("node-data").display().to_string());
        assert_eq!(cfg.database_uri(), "sqlite://node.db");

        fs::remove_file(&path).unwrap();
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_builder_from_yaml() {
        let keypair = KeyPair::random();
        let private_key = keypair.private_key().to_string();
        let yaml = format!(
            "ipv4: true\nprivateKey: \"{private_key}\"\ndataDir: ./tmp_data\ndatabaseUri: storage.db\n"
        );

        let cfg = Builder::new()
            .from(&yaml).unwrap()
            .build().unwrap();

        assert_eq!(cfg.private_key(), &PrivateKey::try_from(private_key.as_str()).unwrap());
        assert_eq!(cfg.data_dir(), "./tmp_data");
        assert_eq!(cfg.database_uri(), "storage.db");
        assert_eq!(cfg.port(), crate::dht::node_config::DEFAULT_DHT_PORT);
    }

    #[test]
    fn test_builder_overrides_loaded_values() {
        let keypair = KeyPair::random();
        let private_key = keypair.private_key().to_string();
        let yaml = format!(
            "ipv4: true\nprivateKey: \"{private_key}\"\nport: 39001\ndataDir: ./tmp_data\ndatabaseUri: storage.db\nlogConsole: false\n"
        );

        let cfg = Builder::new()
            .from(&yaml).unwrap()
            .with_port(40000)
            .with_data_dir("./other_data")
            .with_log_console(true)
            .build()
            .unwrap();

        assert_eq!(cfg.port(), 40000);
        assert_eq!(cfg.data_dir(), "./other_data");
        assert_eq!(cfg.log_console(), true);
        assert_eq!(cfg.database_uri(), "storage.db");
    }

    #[test]
    fn test_builder_programmatic() {
        let keypair = KeyPair::random();

        let cfg = Builder::new()
            .with_host4("localhost")
            .with_private_key(keypair.private_key().clone())
            .with_database_uri("storage.db")
            .build()
            .unwrap();

        assert_eq!(cfg.private_key(), keypair.private_key());
        assert_eq!(cfg.database_uri(), "storage.db");
        assert_eq!(cfg.data_dir(), ".");
        assert_eq!(cfg.port(), crate::dht::node_config::DEFAULT_DHT_PORT);
        assert_eq!(cfg.log_level(), LevelFilter::Info);
        assert!(cfg.bootstrap_nodes().is_empty());
    }

    #[test]
    fn test_builder_missing_private_key_fails() {
        let result = Builder::new()
            .with_database_uri("storage.db")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_missing_database_uri_fails() {
        let keypair = KeyPair::random();
        let result = Builder::new()
            .with_private_key(keypair.private_key().clone())
            .build();

        assert!(result.is_err());
    }
}