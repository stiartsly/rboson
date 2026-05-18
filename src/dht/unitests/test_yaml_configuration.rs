use std::env;
use std::fs;
use log::LevelFilter;

use crate::{
    signature,
};

use crate::dht::{
    cfg::node_config::NodeConfig,
    cfg::yaml_configuration::YamlNodeConfiguration,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_parse_yaml_node_config_from_str() {
        let private_key = signature::KeyPair::random().private_key().to_string();
        let yaml = format!(
            "host4: 203.0.113.10\nport: 39001\nprivateKey: \"{private_key}\"\ndataDir: ./data\nbootstraps:\n  - - 2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k\n    - 203.0.113.5\n    - 39001\nlogger:\n  level: debug\n  logFile: node.log\nenableDeveloperMode: true\n"
        );

        let cfg = YamlNodeConfiguration::from_yaml(&yaml).unwrap();

        assert_eq!(cfg.host4(), Some("203.0.113.10"));
        assert_eq!(cfg.host6(), None);
        assert_eq!(cfg.port(), 39001);
        assert_eq!(cfg.private_key().to_string(), private_key);
        assert_eq!(cfg.data_dir(), "./data");
        assert_eq!(cfg.bootstrap_nodes().len(), 1);
        assert_eq!(cfg.bootstrap_nodes()[0].host(), "203.0.113.5");
        assert_eq!(cfg.bootstrap_nodes()[0].port(), 39001);
        assert_eq!(cfg.log_level(), LevelFilter::Debug);
        assert_eq!(cfg.log_file(), Some("node.log"));
        assert!(cfg.enable_devp());
    }

    #[test]
    fn test_parse_yaml_node_config_expands_env_and_loads_file() {
        let private_key = signature::KeyPair::random().private_key().to_string();
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let temp_dir = env::temp_dir().join(format!("boson-node-yaml-{unique}"));
        fs::create_dir_all(&temp_dir).unwrap();

        let home_dir = temp_dir.join("home");
        fs::create_dir_all(&home_dir).unwrap();

        unsafe {
            env::set_var("HOME", &home_dir);
            env::set_var("PUBLIC_IPV4_ADDRESS", "198.51.100.7");
            env::set_var("NODE_PRIVATE_KEY", &private_key);
        }

        let path = temp_dir.join("node.yaml");
        fs::write(
            &path,
            "host4: ${PUBLIC_IPV4_ADDRESS}\nprivateKey: ${NODE_PRIVATE_KEY}\ndataDir: ~/node-data\n",
        ).unwrap();

        let cfg = YamlNodeConfiguration::load(&path).unwrap();

        assert_eq!(cfg.host4(), Some("198.51.100.7"));
        assert_eq!(cfg.private_key().to_string(), private_key);
        assert_eq!(cfg.data_dir(), home_dir.join("node-data").display().to_string());

        fs::remove_file(&path).unwrap();
        fs::remove_dir_all(&temp_dir).unwrap();
    }
}