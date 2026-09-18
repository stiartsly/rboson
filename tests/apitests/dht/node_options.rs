use boson::{
    dht::{self, NodeOptions},
    signature::KeyPair,
    Id, NodeInfo,
};
use log::LevelFilter;
use serial_test::serial;
use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(content: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "boson-node-options-{}-{}.yaml",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::write(&path, content).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0.as_path()
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn yaml(
    private_key: impl fmt::Display,
    data_dir: &str,
    database_uri: &str,
    log_file: &str,
) -> String {
    format!(
        concat!(
            "ipv4: true\n",
            "ipv6: false\n",
            "port: 39011\n",
            "privateKey: \"{private_key}\"\n",
            "dataDir: \"{data_dir}\"\n",
            "databaseUri: \"{database_uri}\"\n",
            "bootstraps:\n",
            "  - - 2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k\n",
            "    - 203.0.113.5\n",
            "    - 39012\n",
            "logLevel: debug\n",
            "logFile: \"{log_file}\"\n",
            "logConsole: false\n",
            "enableDeveloperMode: true\n",
        ),
        private_key = private_key,
        data_dir = data_dir,
        database_uri = database_uri,
        log_file = log_file,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let sk = KeyPair::random().to_private_key();
        let options = NodeOptions::new(sk.clone());

        assert_eq!(options.private_key(), &sk);
        assert_eq!(options.host4(), None);
        assert_eq!(options.host6(), None);
        assert_eq!(options.port(), dht::DEFAULT_DHT_PORT);
        assert_eq!(options.data_dir(), ".");
        assert_eq!(options.log_file(), None);
        assert_eq!(options.log_console(), true);
        assert_eq!(options.developer_mode(), false);
        assert_eq!(options.bootstrap_nodes().len(), 0);
    }

    #[test]
    fn test_parse_options() {
        let private_key = KeyPair::random().to_private_key();
        let content = yaml(&private_key, "parse-data", "parse.db", "parse.log");
        let config = ConfigFile::new(&content);
        let content = fs::read_to_string(config.path()).unwrap();

        let options = NodeOptions::read(content).unwrap();

        assert_eq!(options.host4().is_some(), true);
        assert_eq!(options.host6(), None);
        assert_eq!(options.port(), 39011);
        assert_eq!(options.private_key(), &private_key);
        assert_eq!(options.data_dir(), "parse-data");
        assert_eq!(options.bootstrap_nodes().len(), 1);
        assert_eq!(
            options.bootstrap_nodes()[0].id().to_base58(),
            "2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k"
        );
        assert_eq!(options.bootstrap_nodes()[0].host(), "203.0.113.5");
        assert_eq!(options.bootstrap_nodes()[0].port(), 39012);
        assert_eq!(options.log_level(), LevelFilter::Debug);
        assert_eq!(options.log_file(), Some("parse.log"));
        assert!(!options.log_console_enabled());
        assert!(options.developer_mode());
    }

    #[test]
    fn test_options_with_funs() {
        let sk = KeyPair::random().to_private_key();
        let bootstrap = NodeInfo::new(Id::random(), "127.0.0.1:39013".parse().unwrap());

        let options = NodeOptions::new(sk.clone())
            .with_host4("127.0.0.1")
            .with_host6("::1")
            .with_port(39014)
            .with_data_dir("set-data")
            .with_bootstrap_nodes(vec![bootstrap.clone()])
            .with_log_level(LevelFilter::Trace)
            .with_log_file("set.log")
            .with_log_console(false)
            .enable_developer_mode();

        assert_eq!(options.host4(), Some("127.0.0.1"));
        assert_eq!(options.host6(), Some("::1"));
        assert_eq!(options.port(), 39014);
        assert_eq!(options.private_key(), &sk);
        assert_eq!(options.data_dir(), "set-data");
        assert_eq!(options.bootstrap_nodes(), &[bootstrap]);
        assert_eq!(options.log_level(), LevelFilter::Trace);
        assert_eq!(options.log_file(), Some("set.log"));
        assert!(!options.log_console_enabled());
        assert!(options.developer_mode());
    }

    #[test]
    fn test_load_then_overridden() {
        let loaded_key = KeyPair::random().to_private_key();
        let replacement_key = KeyPair::random().to_private_key();
        let config = ConfigFile::new(&yaml(&loaded_key, "loaded-data", "loaded.db", "loaded.log"));
        let replacement_bootstrap = NodeInfo::new(Id::random(), "127.0.0.1:39015".parse().unwrap());

        let options = NodeOptions::load(config.path())
            .unwrap()
            .with_host4("192.0.2.1")
            .with_host6("2001:db8::1")
            .with_port(39016)
            .with_private_key(replacement_key.clone())
            .with_data_dir("replacement-data")
            .with_bootstrap_nodes(vec![replacement_bootstrap.clone()])
            .with_log_level(LevelFilter::Warn)
            .with_log_file("replacement.log")
            .enable_log_console()
            .enable_developer_mode();

        assert_eq!(options.host4(), Some("192.0.2.1"));
        assert_eq!(options.host6(), Some("2001:db8::1"));
        assert_eq!(options.port(), 39016);
        assert_eq!(options.private_key(), &replacement_key);
        assert_eq!(options.data_dir(), "replacement-data");
        assert_eq!(options.bootstrap_nodes(), &[replacement_bootstrap]);
        assert_eq!(options.log_level(), LevelFilter::Warn);
        assert_eq!(options.log_file(), Some("replacement.log"));
        assert!(options.log_console_enabled());
        assert!(options.developer_mode());
    }

    #[test]
    #[serial]
    fn test_load_expands_environment_variables() {
        let private_key = KeyPair::random().to_private_key();
        let private_key_value = private_key.to_string();
        let variables = [
            ("BOSON_OPTIONS_TEST_PRIVATE_KEY", private_key_value.as_str()),
            ("BOSON_OPTIONS_TEST_DATA_DIR", "environment-data"),
            ("BOSON_OPTIONS_TEST_DATABASE_URI", "environment.db"),
            ("BOSON_OPTIONS_TEST_LOG_FILE", "environment.log"),
        ];
        for (name, value) in variables {
            unsafe {
                env::set_var(name, value);
            }
        }
        let config = ConfigFile::new(&yaml(
            "${BOSON_OPTIONS_TEST_PRIVATE_KEY}",
            "${BOSON_OPTIONS_TEST_DATA_DIR}",
            "${BOSON_OPTIONS_TEST_DATABASE_URI}",
            "${BOSON_OPTIONS_TEST_LOG_FILE}",
        ));

        let options = NodeOptions::load(config.path()).unwrap();

        assert!(options.host4().is_some());
        assert_eq!(options.host6(), None);
        assert_eq!(options.port(), 39011);
        assert_eq!(options.private_key(), &private_key);
        assert_eq!(options.data_dir(), "environment-data");
        assert_eq!(options.bootstrap_nodes().len(), 1);
        assert_eq!(
            options.bootstrap_nodes()[0].id().to_base58(),
            "2dLbPsaySh9EGWwpgreYiLEPG3NDhaojj7DBBfSsRr6k"
        );
        assert_eq!(options.bootstrap_nodes()[0].host(), "203.0.113.5");
        assert_eq!(options.bootstrap_nodes()[0].port(), 39012);
        assert_eq!(options.log_level(), LevelFilter::Debug);
        assert_eq!(options.log_file(), Some("environment.log"));
        assert!(!options.log_console_enabled());
        assert!(options.developer_mode());

        for (name, _) in variables {
            unsafe {
                env::remove_var(name);
            }
        }
    }
}
