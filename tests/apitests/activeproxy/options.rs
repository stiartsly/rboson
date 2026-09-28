use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

use boson::{activeproxy::Options, signature::KeyPair, Id};
use serial_test::serial;

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(content: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "boson-activeproxy-options-{}-{}.yaml",
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
    userid: impl fmt::Display,
    user_private_key: impl fmt::Display,
    device_private_key: impl fmt::Display,
    service_host: &str,
    upstream_host: &str,
) -> String {
    format!(
        concat!(
            "service:\n",
            "  peerId: {service_peerid}\n",
            "  host: \"{service_host}\"\n",
            "  port: 9091\n",
            "client:\n",
            "  userId: {userid}\n",
            "  userPrivateKey: \"{user_private_key}\"\n",
            "  devicePrivateKey: \"{device_private_key}\"\n",
            "upstream:\n",
            "  host: \"{upstream_host}\"\n",
            "  port: 8081\n",
            "  scheme: http://\n",
            "nameAccess: true\n",
            "announcePeer: true\n",
        ),
        service_peerid = service_peerid,
        userid = userid,
        user_private_key = user_private_key,
        device_private_key = device_private_key,
        service_host = service_host,
        upstream_host = upstream_host,
    )
}

fn assert_loaded_options(
    options: &Options,
    service_peerid: &Id,
    user_key: &KeyPair,
    device_key: &KeyPair,
    service_host: &str,
    upstream_host: &str,
) {
    assert_eq!(options.service_peerid(), service_peerid);
    assert_eq!(options.service_host(), service_host);
    assert_eq!(options.service_port(), 9091);
    assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    assert_eq!(options.device_private_key(), device_key.private_key());
    assert_eq!(options.upstream_host(), upstream_host);
    assert_eq!(options.upstream_port(), 8081);
    assert_eq!(options.upstream_scheme(), "http://");
    assert!(options.is_name_access_enabled());
    assert!(options.is_announce_peer_enabled());
}

#[test]
fn test_default_options() {
    let peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let mut builder = Options::builder(peerid);
    builder
        .with_user_keypair(user_key.clone())
        .with_device_keypair(device_key.clone())
        .with_service_host("127.0.0.1")
        .with_upstream_host("127.0.0.1");
    let options = builder.build().unwrap();

    assert_eq!(options.service_peerid(), &peerid);
    assert_eq!(options.service_host(), "127.0.0.1");
    assert_eq!(options.service_port(), 9090);
    assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    assert_eq!(options.device_private_key(), device_key.private_key());
    assert_eq!(options.upstream_host(), "127.0.0.1");
    assert_eq!(options.upstream_port(), 8080);
    assert_eq!(options.upstream_scheme(), "tcp://");
    assert!(!options.is_name_access_enabled());
    assert!(!options.is_announce_peer_enabled());
}

#[test]
fn test_parse_options() {
    let service_peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let content = yaml(
        service_peerid,
        Id::from(user_key.public_key()),
        user_key.private_key(),
        device_key.private_key(),
        "192.0.2.1",
        "127.0.0.2",
    );

    let parsed = Options::parse(&content).unwrap();
    let read = Options::read(&content).unwrap();

    assert_loaded_options(
        &parsed,
        &service_peerid,
        &user_key,
        &device_key,
        "192.0.2.1",
        "127.0.0.2",
    );
    assert_loaded_options(
        &read,
        &service_peerid,
        &user_key,
        &device_key,
        "192.0.2.1",
        "127.0.0.2",
    );
}

#[test]
fn test_options_with_funs() {
    let peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    let mut builder = Options::builder(peerid);
    builder
        .with_service_host("198.51.100.1")
        .with_service_port(9093)
        .with_userid(Id::random())
        .with_user_keypair(user_key.clone())
        .with_device_keypair(device_key.clone())
        .with_upstream_host("127.0.0.3")
        .with_upstream_port(8082)
        .with_upstream_scheme("https://")
        .with_name_access(true)
        .with_announce_peer(true);
    let options = builder.build().unwrap();

    assert_eq!(options.service_peerid(), &peerid);
    assert_eq!(options.service_host(), "198.51.100.1");
    assert_eq!(options.service_port(), 9093);
    assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    assert_eq!(options.device_private_key(), device_key.private_key());
    assert_eq!(options.upstream_host(), "127.0.0.3");
    assert_eq!(options.upstream_port(), 8082);
    assert_eq!(options.upstream_scheme(), "https://");
    assert!(options.is_name_access_enabled());
    assert!(options.is_announce_peer_enabled());
}

#[test]
fn test_load_options() {
    let loaded_peerid = Id::random();
    let loaded_user_key = KeyPair::random();
    let loaded_device_key = KeyPair::random();
    let config = ConfigFile::new(&yaml(
        loaded_peerid,
        Id::from(loaded_user_key.public_key()),
        loaded_user_key.private_key(),
        loaded_device_key.private_key(),
        "192.0.2.1",
        "127.0.0.2",
    ));
    let options = Options::load(config.path()).unwrap();

    assert_loaded_options(
        &options,
        &loaded_peerid,
        &loaded_user_key,
        &loaded_device_key,
        "192.0.2.1",
        "127.0.0.2",
    );
}

#[test]
#[serial]
fn test_load_and_expand_envioroment_variables() {
    let service_peerid = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let variables = [
        (
            "ACTIVEPROXY_OPTIONS_TEST_PEERID",
            service_peerid.to_string(),
        ),
        (
            "ACTIVEPROXY_OPTIONS_TEST_USERID",
            Id::from(user_key.public_key()).to_string(),
        ),
        (
            "ACTIVEPROXY_OPTIONS_TEST_USER_KEY",
            user_key.private_key().to_string(),
        ),
        (
            "ACTIVEPROXY_OPTIONS_TEST_DEVICE_KEY",
            device_key.private_key().to_string(),
        ),
        (
            "ACTIVEPROXY_OPTIONS_TEST_SERVICE_HOST",
            "192.0.2.2".to_string(),
        ),
        (
            "ACTIVEPROXY_OPTIONS_TEST_UPSTREAM_HOST",
            "127.0.0.5".to_string(),
        ),
    ];
    for (name, value) in &variables {
        unsafe {
            env::set_var(name, value);
        }
    }
    let config = ConfigFile::new(&yaml(
        "${ACTIVEPROXY_OPTIONS_TEST_PEERID}",
        "${ACTIVEPROXY_OPTIONS_TEST_USERID}",
        "${ACTIVEPROXY_OPTIONS_TEST_USER_KEY}",
        "${ACTIVEPROXY_OPTIONS_TEST_DEVICE_KEY}",
        "${ACTIVEPROXY_OPTIONS_TEST_SERVICE_HOST}",
        "${ACTIVEPROXY_OPTIONS_TEST_UPSTREAM_HOST}",
    ));

    let options = Options::load(config.path()).unwrap();

    assert_loaded_options(
        &options,
        &service_peerid,
        &user_key,
        &device_key,
        "192.0.2.2",
        "127.0.0.5",
    );
    for (name, _) in &variables {
        unsafe {
            env::remove_var(name);
        }
    }
}
