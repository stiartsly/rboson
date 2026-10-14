use std::{
    env, fmt, fs,
    process,
    path::{Path, PathBuf},
};
use boson::{
    signature::KeyPair,
    Id, PeerInfo,
};
use boson::messaging::{
    ChannelListener,
    ConnectionListener,
    ContactListener,
    FriendRequestListener,
    MessageListener,
    Message,
    Options, OptionsBuilder,
    SessionListener,
    SessionInfo,
};

use serial_test::serial;

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(content: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "boson-messaging-options-{}-{}.yaml",
            process::id(),
            rand::random::<u64>()
        ));
        fs::write(&path, content).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        self.0.as_path()
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Clone)]
struct DatabaseYaml {
    uri: String,
    pool_size: Option<usize>,
    schema: Option<String>,
}

impl DatabaseYaml {
    fn new(uri: impl fmt::Display, pool_size: usize, schema: impl fmt::Display) -> Self {
        Self {
            uri: uri.to_string(),
            pool_size: Some(pool_size),
            schema: Some(schema.to_string()),
        }
    }
}

#[derive(Clone)]
struct ConfigYaml {
    peer_id: String,
    endpoint: Option<String>,
    user_id: Option<String>,
    user_private_key: String,
    device_id: Option<String>,
    device_private_key: String,
    data_dir: Option<String>,
    database: Option<DatabaseYaml>,
    log_level: Option<String>,
    log_file: Option<String>,
    log_console: Option<bool>,
}

impl ConfigYaml {
    fn new(
        peer_id: impl fmt::Display,
        user_key: &KeyPair,
        device_key: &KeyPair
    ) -> Self {
        Self {
            peer_id: peer_id.to_string(),
            endpoint: Some("mqtts://10.0.0.1:8883".to_string()),
            user_id: None,
            user_private_key: user_key.private_key().to_base58(),
            device_id: None,
            device_private_key: device_key.private_key().to_base58(),
            data_dir: None,
            database: None,
            log_level: None,
            log_file: None,
            log_console: None,
        }
    }

    fn endpoint(mut self, endpoint: impl fmt::Display) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    fn without_endpoint(mut self) -> Self {
        self.endpoint = None;
        self
    }

    fn user_id(mut self, user_id: impl fmt::Display) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    fn device_id(mut self, device_id: impl fmt::Display) -> Self {
        self.device_id = Some(device_id.to_string());
        self
    }

    fn user_private_key(mut self, private_key: impl fmt::Display) -> Self {
        self.user_private_key = private_key.to_string();
        self
    }

    fn device_private_key(mut self, private_key: impl fmt::Display) -> Self {
        self.device_private_key = private_key.to_string();
        self
    }

    fn data_dir(mut self, data_dir: impl fmt::Display) -> Self {
        self.data_dir = Some(data_dir.to_string());
        self
    }

    fn database(mut self, uri: impl fmt::Display, pool_size: usize, schema: impl fmt::Display) -> Self {
        self.database = Some(DatabaseYaml::new(uri, pool_size, schema));
        self
    }

    fn database_block(mut self, database: DatabaseYaml) -> Self {
        self.database = Some(database);
        self
    }

    fn log_level(mut self, level: impl fmt::Display) -> Self {
        self.log_level = Some(level.to_string());
        self
    }

    fn log_file(mut self, file: impl fmt::Display) -> Self {
        self.log_file = Some(file.to_string());
        self
    }

    fn log_console(mut self, enabled: bool) -> Self {
        self.log_console = Some(enabled);
        self
    }

    fn render(&self) -> String {
        let mut lines = vec![
            "service:".to_string(),
            format!("  peerId: {}", self.peer_id),
        ];
        if let Some(endpoint) = &self.endpoint {
            lines.push(format!("  endpoint: \"{endpoint}\""));
        }

        lines.push("client:".to_string());
        if let Some(user_id) = &self.user_id {
            lines.push(format!("  userId: {user_id}"));
        }
        lines.push(format!("  userPrivateKey: \"{}\"", self.user_private_key));
        if let Some(device_id) = &self.device_id {
            lines.push(format!("  deviceId: {device_id}"));
        }
        lines.push(format!("  devicePrivateKey: \"{}\"", self.device_private_key));

        if let Some(data_dir) = &self.data_dir {
            lines.push(format!("dataDir: \"{data_dir}\""));
        }
        if let Some(database) = &self.database {
            lines.push("database:".to_string());
            lines.push(format!("  uri: \"{}\"", database.uri));
            if let Some(pool_size) = database.pool_size {
                lines.push(format!("  poolSize: {pool_size}"));
            }
            if let Some(schema) = &database.schema {
                lines.push(format!("  schema: \"{schema}\""));
            }
        }
        if let Some(level) = &self.log_level {
            lines.push(format!("logLevel: \"{level}\""));
        }
        if let Some(file) = &self.log_file {
            lines.push(format!("logFile: \"{file}\""));
        }
        if let Some(console) = self.log_console {
            lines.push(format!("logConsole: {console}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

#[derive(Default)]
struct EnvVars(Vec<String>);

impl EnvVars {
    fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        unsafe {
            env::set_var(&name, value.into());
        }
        self.0.push(name);
    }
}

impl Drop for EnvVars {
    fn drop(&mut self) {
        for name in &self.0 {
            unsafe {
                env::remove_var(name);
            }
        }
    }
}

fn complete_builder(peer_id: Id, user_key: &KeyPair, device_key: &KeyPair) -> OptionsBuilder {
    let mut builder = Options::builder();
    builder.with_peer_id(peer_id);
    builder.with_peer_endpoint("mqtts://10.0.0.1:8883").unwrap();
    builder.with_user_keypair(user_key.clone());
    builder.with_device_keypair(device_key.clone());
    builder
}

fn options_with_database(data_dir: &Path, uri: &str) -> Options {
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let mut builder = complete_builder(Id::random(), &user_key, &device_key);
    builder.with_data_dir(data_dir);
    builder.with_database(uri, 2).unwrap();
    builder.build().unwrap()
}

fn assert_identity(
    options: &Options,
    peer_id: &Id,
    endpoint: &str,
    user_key: &KeyPair,
    device_key: &KeyPair,
) {
    assert_eq!(options.peer_id(), peer_id);
    assert_eq!(options.service_peerid(), peer_id);
    assert_eq!(options.peer_endpoint().as_str(), endpoint);
    assert_eq!(
        options.service_endpoint().map(url::Url::as_str),
        Some(endpoint)
    );

    assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    assert_eq!(options.user_key().private_key(), user_key.private_key());
    assert_eq!(options.user_private_key(), user_key.private_key());

    assert_eq!(options.device_id(), &Id::from(device_key.public_key()));
    assert_eq!(options.device_key().private_key(), device_key.private_key());
    assert_eq!(options.device_private_key(), device_key.private_key());
}

mod builder {
    use super::*;

    #[test]
    fn test_options_builder_completeness() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();

        let mut builder = OptionsBuilder::new();
        let rc = builder.build();
        assert!(rc.is_err());

        let error = rc.err().unwrap().to_string();
        assert!(error.contains("service peer id is required"));

        builder.with_peer_id(peer_id);
        let rc = builder.build();
        assert!(rc.is_err());

        let error = rc.err().unwrap().to_string();
        assert!(error.contains("service peer endpoint is required"));

        builder.with_peer_endpoint("mqtts://10.0.0.1:8883").unwrap();
        let rc = builder.build();
        assert!(rc.is_err());

        let error = rc.err().unwrap().to_string();
        assert!(error.contains("user_key is required"));

        builder.with_user_keypair(user_key.clone());
        let rc = builder.build();
        assert!(rc.is_err());

        let error = rc.err().unwrap().to_string();
        assert!(error.contains("device_key is required"));

        builder.with_device_keypair(device_key.clone());
        let rc = builder.build();
        assert!(rc.is_ok());
    }

    #[test]
    fn test_options_builder_defaults() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let builder = complete_builder(peer_id, &user_key, &device_key);
        let current_dir = env::current_dir().unwrap();

        let rc = builder.build();
        assert!(rc.is_ok());

        let options = rc.unwrap();
        assert_identity(
            &options,
            &peer_id,
            "mqtts://10.0.0.1:8883",
            &user_key,
            &device_key,
        );
        assert_eq!(options.data_dir(), current_dir);
        assert_eq!(options.database_uri(), "jdbc:sqlite:messaging.db");
        assert_eq!(options.database_pool_size(), 1);
        assert_eq!(options.database_schema(), "public");
        assert_eq!(options.database_path(), current_dir.join("messaging.db"));
        assert_eq!(options.log_level(), log::LevelFilter::Info);
        assert_eq!(options.log_file(), None);
        assert!(!options.log_console());

        // Building twice must not consume or clear the secrets.
        let rebuilt = builder.build().unwrap();
        assert_eq!(rebuilt.user_id(), options.user_id());
        assert_eq!(rebuilt.device_id(), options.device_id());
        assert_eq!(rebuilt.database_uri(), options.database_uri());
    }

    #[test]
    fn test_options_builder_constructors() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();

        let mut from_default = OptionsBuilder::default();
        from_default.with_peer_id(peer_id);
        from_default.with_peer_endpoint("mqtts://10.0.0.1:8883").unwrap();
        from_default.with_user_keypair(user_key.clone());
        from_default.with_device_keypair(device_key.clone());
        let from_default = from_default.build().unwrap();

        let mut from_options = Options::builder();
        from_options.with_peer_id(peer_id);
        from_options.with_peer_endpoint("mqtts://10.0.0.1:8883").unwrap();
        from_options.with_user_keypair(user_key.clone());
        from_options.with_device_keypair(device_key.clone());
        let from_options = from_options.build().unwrap();

        assert_eq!(from_default.data_dir(), from_options.data_dir());
        assert_eq!(from_default.user_id(), from_options.user_id());
        assert_eq!(from_default.device_id(), from_options.device_id());
        assert_eq!(
            from_default.peer_endpoint().as_str(),
            from_options.peer_endpoint().as_str()
        );
    }

    #[test]
    fn test_options_builder_main_setters() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let data_dir = env::temp_dir().join("boson-options-primary");

        let mut builder = Options::builder();
        builder.with_peer_id(peer_id);
        builder.with_peer_endpoint("mqtts://10.0.0.1:8883").unwrap();
        builder.with_user_keypair(user_key.clone());
        builder.with_device_keypair(device_key.clone());
        builder.with_data_dir(&data_dir);
        builder.with_database("jdbc:sqlite:primary.db", 4).unwrap();
        builder.with_database_schema("primary_schema");
        builder.with_log_level(log::LevelFilter::Debug);
        builder.with_log_file("/tmp/boson-options-primary.log");
        builder.with_log_console(true);

        let options = builder.build().unwrap();
        assert_identity(
            &options,
            &peer_id,
            "mqtts://10.0.0.1:8883",
            &user_key,
            &device_key,
        );
        assert_eq!(options.data_dir(), data_dir);
        assert_eq!(options.database_uri(), "jdbc:sqlite:primary.db");
        assert_eq!(options.database_pool_size(), 4);
        assert_eq!(options.database_schema(), "primary_schema");
        assert_eq!(options.database_path(), data_dir.join("primary.db"));
        assert_eq!(options.log_level(), log::LevelFilter::Debug);
        assert_eq!(options.log_file(), Some("/tmp/boson-options-primary.log"));
        assert!(options.log_console());
    }

    #[test]
    fn test_builder_with_alias_setters() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let data_dir = env::temp_dir().join("boson-options-aliases");

        let mut builder = Options::builder();
        builder.with_peer_id(peer_id);
        builder.with_peer_endpoint("mqtts://10.0.0.2:8883").unwrap();
        builder.with_user_private_key(user_key.private_key().clone());
        builder.with_device_private_key(device_key.private_key().clone());
        builder.with_data_dir(&data_dir);
        builder.with_database_uri("jdbc:sqlite:alias.db").unwrap();
        builder.with_database_pool_size(7);
        builder.with_database_schema("alias_schema");

        let options = builder.build().unwrap();
        assert_identity(
            &options,
            &peer_id,
            "mqtts://10.0.0.2:8883",
            &user_key,
            &device_key,
        );
        assert_eq!(options.data_dir(), data_dir);
        assert_eq!(options.database_uri(), "jdbc:sqlite:alias.db");
        assert_eq!(options.database_pool_size(), 7);
        assert_eq!(options.database_schema(), "alias_schema");
    }

    #[test]
    fn test_builder_with_peer_info() {
        let peer_key = KeyPair::random();
        let peer = PeerInfo::builder("mqtts://10.0.0.3:8883")
            .with_key(peer_key)
            .build()
            .unwrap();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();

        let mut builder = Options::builder();
        builder.with_peer(&peer).unwrap();
        builder.with_user_keypair(user_key.clone());
        builder.with_device_keypair(device_key.clone());

        let options = builder.build().unwrap();
        assert_eq!(options.peer_id(), peer.id());
        assert_eq!(options.peer_endpoint().as_str(), peer.endpoint());
        assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    }

    #[test]
    fn test_options_builder_endpoint_variants() {
        let mut builder = Options::builder();

        let url = url::Url::parse("mqtts://10.0.0.1:8883").unwrap();
        assert!(builder.with_peer_endpoint_url(url.clone()).is_ok());
        assert!(builder.with_peer_endpoint("mqtts://10.0.0.1:8883").is_ok());
        assert!(builder.with_peer_endpoint("mqtt://localhost:1883").is_ok());

        let invalid_scheme = builder
            .with_peer_endpoint("tcp://10.0.0.1:8883")
            .err()
            .unwrap()
            .to_string();
        assert!(invalid_scheme.contains("Invalid endpoint scheme"));
        assert!(builder.with_peer_endpoint("http://10.0.0.1:8883").is_err());

        let missing_port = builder
            .with_peer_endpoint("mqtts://10.0.0.1")
            .err()
            .unwrap()
            .to_string();
        assert!(missing_port.contains("port"));

        assert!(builder.with_peer_endpoint("mqtts:///missing-host").is_err());
        assert!(builder.with_peer_endpoint("not a url").is_err());
        assert!(builder.with_peer_endpoint("").is_err());

        let invalid_url = url::Url::parse("ws://10.0.0.1:8883").unwrap();
        assert!(builder.with_peer_endpoint_url(invalid_url).is_err());
        let invalid_service_url = url::Url::parse("https://10.0.0.1:8883").unwrap();
        assert!(builder.with_peer_endpoint_url(invalid_service_url).is_err());
    }

    #[test]
    fn test_options_builder_empty_db_and_data_dir() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();

        let mut builder = Options::builder();
        assert!(builder.with_database("", 1).is_err());
        assert!(builder.with_database("   ", 1).is_err());
        assert!(builder.with_database_uri("").is_err());
        assert!(builder.with_database_uri("  ").is_err());

        let mut builder = complete_builder(peer_id, &user_key, &device_key);
        builder.with_data_dir("");
        let error = builder.build().err().unwrap().to_string();
        assert!(error.contains("data_dir must not be empty"));
    }

    #[test]
    fn test_options_builder_with_listeners() {
        struct TestConnectionListener;
        impl ConnectionListener for TestConnectionListener {
            fn on_ready(&self) {}
        }

        struct TestMessageListener;
        impl MessageListener for TestMessageListener {
            fn on_message(&self, _message: &dyn Message) {}
        }

        struct TestChannelListener;
        impl ChannelListener for TestChannelListener {}

        struct TestContactListener;
        impl ContactListener for TestContactListener {}

        struct TestSessionListener;
        impl SessionListener for TestSessionListener {
            fn on_new_session(&self, _session: &SessionInfo) {}
        }

        struct TestFriendRequestListener;
        impl FriendRequestListener for TestFriendRequestListener {}

        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let mut builder = complete_builder(Id::random(), &user_key, &device_key);
        builder.with_connection_listener(TestConnectionListener);
        builder.with_message_listener(TestMessageListener);
        builder.with_channel_listener(TestChannelListener);
        builder.with_contact_listener(TestContactListener);
        builder.with_session_listener(TestSessionListener);
        builder.with_friend_request_listener(TestFriendRequestListener);

        let options = builder.build().unwrap();
        assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
        assert_eq!(options.device_id(), &Id::from(device_key.public_key()));
    }

    #[test]
    fn test_cloned_builder() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let builder = complete_builder(peer_id, &user_key, &device_key);

        let cloned = builder.clone();
        drop(builder);

        let options = cloned.build().unwrap();
        assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
        assert_eq!(options.device_id(), &Id::from(device_key.public_key()));
    }
}

// ---------------------------------------------------------------------------
// Options::parse / OptionsBuilder::parse
// ---------------------------------------------------------------------------

mod parse {
    use super::*;

    #[test]
    fn test_parse_full_config() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let data_dir = env::temp_dir().join("boson-options-parse");
        let config = ConfigYaml::new(peer_id, &user_key, &device_key)
            .user_id(Id::from(user_key.public_key()))
            .device_id(Id::from(device_key.public_key()))
            .data_dir(data_dir.display())
            .database("jdbc:sqlite:parse.db", 3, "boson")
            .log_level("debug")
            .log_file("/tmp/boson-options-parse.log")
            .log_console(false);

        let options = Options::parse(config.render()).unwrap();
        assert_identity(
            &options,
            &peer_id,
            "mqtts://10.0.0.1:8883",
            &user_key,
            &device_key,
        );
        assert_eq!(options.data_dir(), data_dir);
        assert_eq!(options.database_uri(), "jdbc:sqlite:parse.db");
        assert_eq!(options.database_pool_size(), 3);
        assert_eq!(options.database_schema(), "boson");
        assert_eq!(options.database_path(), data_dir.join("parse.db"));
        assert_eq!(options.log_level(), log::LevelFilter::Debug);
        assert_eq!(options.log_file(), Some("/tmp/boson-options-parse.log"));
        assert!(!options.log_console());
    }

    #[test]
    fn test_parse_minimal_config() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(peer_id, &user_key, &device_key);
        let current_dir = env::current_dir().unwrap();

        let options = Options::parse(&config.render()).unwrap();
        assert_eq!(options.data_dir(), current_dir);
        assert_eq!(options.database_uri(), "jdbc:sqlite:messaging.db");
        assert_eq!(options.database_pool_size(), 1);
        assert_eq!(options.database_schema(), "public");
        assert_eq!(options.database_path(), current_dir.join("messaging.db"));
        assert_eq!(options.log_level(), log::LevelFilter::Info);
        assert_eq!(options.log_file(), None);
        assert!(options.log_console());
    }

    #[test]
    fn test_parse_database_block() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(peer_id, &user_key, &device_key)
            .database_block(DatabaseYaml::new("jdbc:sqlite:block.db", 5, "block_schema"));

        let options = Options::parse(config.render()).unwrap();
        assert_eq!(options.database_uri(), "jdbc:sqlite:block.db");
        assert_eq!(options.database_pool_size(), 5);
        assert_eq!(options.database_schema(), "block_schema");
    }

    #[test]
    fn test_parse_mismatched_userid() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .user_id(Id::random());

        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("userId does not match userPrivateKey"));
    }

    #[test]
    fn test_parse_mismatched_deviceid() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .device_id(Id::random());

        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("deviceId does not match devicePrivateKey"));
    }

    #[test]
    fn test_parse_invalid_private_keys() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .user_private_key("not-a-private-key");
        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("invalid user private key"));

        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .device_private_key("not-a-private-key");
        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("invalid device private key"));
    }

    #[test]
    fn test_parse_invalid_endpoint() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();

        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .without_endpoint();
        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("service peer endpoint is required"));

        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .endpoint("tcp://10.0.0.1:8883");
        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("Invalid endpoint scheme"));

        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .endpoint("mqtts://10.0.0.1");
        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("port"));
    }

    #[test]
    fn test_parse_missing_required_sections() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let client = format!(
            "client:\n  userPrivateKey: \"{}\"\n  devicePrivateKey: \"{}\"\n",
            user_key.private_key().to_base58(),
            device_key.private_key().to_base58(),
        );

        let missing_service = client.clone();
        assert!(Options::parse(&missing_service).is_err());

        let missing_client = format!(
            "service:\n  peerId: {}\n  endpoint: \"mqtts://10.0.0.1:8883\"\n",
            Id::random()
        );
        assert!(Options::parse(&missing_client).is_err());

        let missing_peer_id = format!("service:\n  endpoint: \"mqtts://10.0.0.1:8883\"\n{client}");
        assert!(Options::parse(&missing_peer_id).is_err());

        let missing_user_key = format!(
            "service:\n  peerId: {}\n  endpoint: \"mqtts://10.0.0.1:8883\"\nclient:\n  devicePrivateKey: \"{}\"\n",
            Id::random(),
            device_key.private_key().to_base58(),
        );
        assert!(Options::parse(&missing_user_key).is_err());

        let missing_device_key = format!(
            "service:\n  peerId: {}\n  endpoint: \"mqtts://10.0.0.1:8883\"\nclient:\n  userPrivateKey: \"{}\"\n",
            Id::random(),
            user_key.private_key().to_base58(),
        );
        assert!(Options::parse(&missing_device_key).is_err());

        assert!(Options::parse("service: [not-a-mapping]").is_err());
    }

    #[test]
    fn test_parse_empty_database_uri() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .database("", 1, "public");

        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("Database URI is empty"));
    }

    #[test]
    fn test_parse_invalid_log_level() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .log_level("verbose");

        let error = Options::parse(config.render()).err().unwrap().to_string();
        assert!(error.contains("invalid logLevel"));
    }

    #[test]
    fn test_parse_expands_home_dir() {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .data_dir("~/boson-options-home");
        let options = Options::parse(config.render()).unwrap();
        assert_eq!(options.data_dir(), home.join("boson-options-home"));

        let config = ConfigYaml::new(Id::random(), &user_key, &device_key)
            .data_dir("~");
        let options = Options::parse(config.render()).unwrap();
        assert_eq!(options.data_dir(), home);
    }

    #[test]
    #[serial]
    fn test_parse_expands_environment_variables() {
        let suffix = rand::random::<u64>();
        let peer_var = format!("BOSON_OPTIONS_PEER_{suffix}");
        let endpoint_var = format!("BOSON_OPTIONS_ENDPOINT_{suffix}");
        let user_key_var = format!("BOSON_OPTIONS_USER_KEY_{suffix}");
        let device_key_var = format!("BOSON_OPTIONS_DEVICE_KEY_{suffix}");
        let data_dir_var = format!("BOSON_OPTIONS_DATA_DIR_{suffix}");

        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let data_dir = env::temp_dir().join(format!("boson-options-env-{suffix}"));
        let mut vars = EnvVars::default();
        vars.set(&peer_var, peer_id.to_string());
        vars.set(&endpoint_var, "mqtts://10.0.0.4:8883");
        vars.set(&user_key_var, user_key.private_key().to_base58());
        vars.set(&device_key_var, device_key.private_key().to_base58());
        vars.set(&data_dir_var, data_dir.display().to_string());

        let content = format!(
            concat!(
                "service:\n",
                "  peerId: ${{{peer_var}}}\n",
                "  endpoint: \"${{{endpoint_var}}}\"\n",
                "client:\n",
                "  userPrivateKey: \"${{{user_key_var}}}\"\n",
                "  devicePrivateKey: \"${{{device_key_var}}}\"\n",
                "dataDir: \"${{{data_dir_var}}}\"\n",
            ),
            peer_var = peer_var,
            endpoint_var = endpoint_var,
            user_key_var = user_key_var,
            device_key_var = device_key_var,
            data_dir_var = data_dir_var,
        );

        let options = Options::parse(content).unwrap();
        assert_identity(
            &options,
            &peer_id,
            "mqtts://10.0.0.4:8883",
            &user_key,
            &device_key,
        );
        assert_eq!(options.data_dir(), data_dir);
    }

    #[test]
    fn test_parse_environ_vars_failures() {
        let name = format!("BOSON_OPTIONS_UNTERMINATED_{}", rand::random::<u64>());
        let unterminated = format!("service:\n  peerId: ${{{name}");
        let error = Options::parse(unterminated).err().unwrap().to_string();
        assert!(error.contains("unterminated environment variable reference"));

        let error = Options::parse("service:\n  peerId: ${}")
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("empty environment variable name"));

        let missing = format!("BOSON_OPTIONS_MISSING_{}", rand::random::<u64>());
        let content = format!("service:\n  peerId: ${{{missing}}}");
        let error = Options::parse(content).err().unwrap().to_string();
        assert!(error.contains("is not set"));
    }

    #[test]
    fn test_builder_parse_validation() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(peer_id, &user_key, &device_key);

        let builder = OptionsBuilder::parse(config.render()).unwrap();
        let options = builder.build().unwrap();
        assert_eq!(options.peer_id(), &peer_id);

        let config = ConfigYaml::new(peer_id, &user_key, &device_key).without_endpoint();
        let builder = OptionsBuilder::parse(config.render()).unwrap();
        let error = builder.build().err().unwrap().to_string();
        assert!(error.contains("service peer endpoint is required"));
    }
}

// ---------------------------------------------------------------------------
// Options::load / OptionsBuilder::load
// ---------------------------------------------------------------------------

mod load {
    use super::*;

    #[test]
    fn test_load_reads_config_file() {
        let peer_id = Id::random();
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let config = ConfigYaml::new(peer_id, &user_key, &device_key)
            .data_dir("/tmp/boson-options-load")
            .database("jdbc:sqlite:load.db", 2, "load_schema")
            .log_level("warn")
            .log_console(false);
        let file = ConfigFile::new(&config.render());

        let options = Options::load(file.path()).unwrap();
        assert_identity(
            &options,
            &peer_id,
            "mqtts://10.0.0.1:8883",
            &user_key,
            &device_key,
        );
        assert_eq!(options.data_dir(), Path::new("/tmp/boson-options-load"));
        assert_eq!(options.database_uri(), "jdbc:sqlite:load.db");
        assert_eq!(options.database_pool_size(), 2);
        assert_eq!(options.database_schema(), "load_schema");
        assert_eq!(options.log_level(), log::LevelFilter::Warn);

        let options = OptionsBuilder::load(file.path()).unwrap().build().unwrap();
        assert_eq!(options.peer_id(), &peer_id);
        assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    }

    #[test]
    fn test_load_missing_file() {
        let path = env::temp_dir().join(format!(
            "boson-options-missing-{}-{}.yaml",
            std::process::id(),
            rand::random::<u64>()
        ));
        let error = Options::load(&path).err().unwrap().to_string();
        assert!(error.contains("Error reading config"));
        assert!(OptionsBuilder::load(&path).is_err());
    }
}
