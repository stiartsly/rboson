use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

use boson::{messaging::Options, signature::KeyPair, Id};
use serial_test::serial;

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(content: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "boson-messaging-options-{}-{}.yaml",
            std::process::id(),
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

fn yaml(
    service_peerid: impl fmt::Display,
    endpoint: &str,
    user_private_key: impl fmt::Display,
    device_private_key: impl fmt::Display,
    data_dir: &str,
    database_uri: &str,
    pool_size: usize,
    schema: &str,
) -> String {
    format!(
        concat!(
            "service:\n",
            "  peerId: {service_peerid}\n",
            "  endpoint: \"{endpoint}\"\n",
            "client:\n",
            "  userPrivateKey: \"{user_private_key}\"\n",
            "  devicePrivateKey: \"{device_private_key}\"\n",
            "dataDir: \"{data_dir}\"\n",
            "database:\n",
            "  uri: \"{database_uri}\"\n",
            "  poolSize: {pool_size}\n",
            "  schema: \"{schema}\"\n",
        ),
        service_peerid = service_peerid,
        endpoint = endpoint,
        user_private_key = user_private_key,
        device_private_key = device_private_key,
        data_dir = data_dir,
        database_uri = database_uri,
        pool_size = pool_size,
        schema = schema,
    )
}

#[test]
fn test_default_options() {
    let options = Options::new();

    assert_eq!(options.service_peerid(), None);
    assert_eq!(options.service_endpoint(), None);
    assert_eq!(options.user_id(), None);
    assert!(options.user_key().is_none());
    assert!(options.user_private_key().is_none());
    assert_eq!(options.device_id(), None);
    assert!(options.device_key().is_none());
    assert!(options.device_private_key().is_none());
    assert_eq!(options.database_uri(), "jdbc:sqlite:messaging.db");
    assert_eq!(options.database_pool_size(), 0);
    assert_eq!(options.database_schema_name(), None);
    assert_eq!(options.data_dir(), Path::new("."));
}

#[test]
fn test_options_with_builders() {
    let peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    let options = Options::new()
        .with_service_peerid(peerid)
        .with_service_endpoint("mqtts://10.0.0.1:8883")
        .unwrap()
        .with_user_keypair(user_key.clone())
        .with_device_keypair(device_key.clone())
        .with_data_dir("/tmp/photon-messaging-test")
        .with_database("postgresql://localhost:5432/test", 4)
        .unwrap()
        .with_database_schema_name("photon");

    assert_eq!(options.service_peerid(), Some(&peerid));
    assert_eq!(
        options.service_endpoint().map(url::Url::as_str),
        Some("mqtts://10.0.0.1:8883")
    );
    assert_eq!(options.user_id(), Some(&Id::from(user_key.public_key())));
    assert_eq!(options.user_private_key(), Some(user_key.private_key()));
    assert_eq!(
        options.device_id(),
        Some(&Id::from(device_key.public_key()))
    );
    assert_eq!(options.device_private_key(), Some(device_key.private_key()));
    assert_eq!(options.data_dir(), Path::new("/tmp/photon-messaging-test"));
    assert_eq!(options.database_uri(), "postgresql://localhost:5432/test");
    assert_eq!(options.database_pool_size(), 4);
    assert_eq!(options.database_schema_name(), Some("photon"));
    assert!(options.check_completeness().is_ok());
}

#[test]
fn test_parse_options() {
    let service_peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let content = yaml(
        &service_peerid,
        "mqtts://192.168.8.80:8883",
        user_key.private_key().to_base58(),
        device_key.private_key().to_base58(),
        "/tmp/messaging-data",
        "jdbc:sqlite:custom.db",
        2,
        "custom_schema",
    );

    let options = Options::parse(&content).unwrap();
    assert_eq!(options.service_peerid(), Some(&service_peerid));
    assert_eq!(
        options.service_endpoint().map(url::Url::as_str),
        Some("mqtts://192.168.8.80:8883")
    );
    assert_eq!(options.user_id(), Some(&Id::from(user_key.public_key())));
    assert_eq!(options.user_private_key(), Some(user_key.private_key()));
    assert_eq!(
        options.device_id(),
        Some(&Id::from(device_key.public_key()))
    );
    assert_eq!(options.device_private_key(), Some(device_key.private_key()));
    assert_eq!(options.data_dir(), Path::new("/tmp/messaging-data"));
    assert_eq!(options.database_uri(), "jdbc:sqlite:custom.db");
    assert_eq!(options.database_pool_size(), 2);
    assert_eq!(options.database_schema_name(), Some("custom_schema"));
    assert!(options.check_completeness().is_ok());
}

#[test]
fn test_load_options_from_file() {
    let service_peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let content = yaml(
        &service_peerid,
        "mqtts://10.0.0.1:8883",
        user_key.private_key().to_base58(),
        device_key.private_key().to_base58(),
        "/tmp/messaging-file",
        "jdbc:sqlite:test.db",
        1,
        "photon",
    );

    let file = ConfigFile::new(&content);
    let options = Options::load(file.path()).unwrap();
    assert_eq!(options.service_peerid(), Some(&service_peerid));
    assert_eq!(options.data_dir(), Path::new("/tmp/messaging-file"));
    assert_eq!(options.user_id(), Some(&Id::from(user_key.public_key())));
    assert!(options.check_completeness().is_ok());
}

#[test]
#[serial]
fn test_env_var_expansion_in_yaml() {
    let service_peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    let endpoint_var = format!("TEST_ENDPOINT_{}", rand::random::<u32>());
    unsafe {
        env::set_var(&endpoint_var, "mqtts://127.0.0.1:8883");
    }

    let raw = format!(
        concat!(
            "service:\n",
            "  peerId: {service_peerid}\n",
            "  endpoint: \"${{{endpoint_var}}}\"\n",
            "client:\n",
            "  userPrivateKey: \"{user_pk}\"\n",
            "  devicePrivateKey: \"{device_pk}\"\n",
        ),
        service_peerid = service_peerid,
        endpoint_var = endpoint_var,
        user_pk = user_key.private_key().to_base58(),
        device_pk = device_key.private_key().to_base58(),
    );

    let options = Options::parse(&raw).unwrap();
    assert_eq!(
        options.service_endpoint().map(url::Url::as_str),
        Some("mqtts://127.0.0.1:8883")
    );

    unsafe {
        env::remove_var(&endpoint_var);
    }
}

#[test]
fn test_check_completeness() {
    let peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    // Incomplete: default options has no user or device key
    let incomplete = Options::new();

    assert!(incomplete.check_completeness().is_err());

    // Incomplete: only user key, no device key
    let no_device = Options::new()
        .with_service_peerid(peerid)
        .with_user_keypair(user_key.clone());
    assert!(no_device.check_completeness().is_err());

    // Complete: has both user and device keys
    let complete = Options::new()
        .with_service_peerid(peerid)
        .with_user_keypair(user_key.clone())
        .with_device_keypair(device_key.clone());
    assert!(complete.check_completeness().is_ok());

    // Inconsistent user_id
    let mut mismatched_user = complete.clone();
    mismatched_user.user_id = Some(Id::random());
    assert!(mismatched_user.check_completeness().is_err());

    // Inconsistent device_id
    let mut mismatched_device = complete.clone();
    mismatched_device.device_id = Some(Id::random());
    assert!(mismatched_device.check_completeness().is_err());

    // Empty database URI
    let mut empty_db = complete.clone();
    empty_db.database_uri = "".to_string();
    assert!(empty_db.check_completeness().is_err());
}

#[test]
fn test_try_from_str() {
    let service_peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let content = yaml(
        &service_peerid,
        "mqtts://10.0.0.1:8883",
        user_key.private_key().to_base58(),
        device_key.private_key().to_base58(),
        "/tmp/messaging-tryfrom",
        "jdbc:sqlite:test.db",
        1,
        "photon",
    );

    let options: Options = Options::parse(content).unwrap();
    assert_eq!(options.service_peerid(), Some(&service_peerid));
    assert_eq!(options.data_dir(), Path::new("/tmp/messaging-tryfrom"));
}
