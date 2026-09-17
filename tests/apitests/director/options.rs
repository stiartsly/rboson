use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

use boson::{
    director::DirectorOptions,
    signature::KeyPair,
    Id,
};
use serial_test::serial;

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(content: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "boson-director-options-{}-{}.yaml",
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
    url: &str,
    node_id: impl std::fmt::Display,
    user_id: impl std::fmt::Display,
    user_key: impl std::fmt::Display,
    device_key: impl std::fmt::Display,
) -> String {
    format!(
        concat!(
            "director:\n",
            "  url: \"{url}\"\n",
            "  nodeId: {node_id}\n",
            "  insecure: true\n",
            "user:\n",
            "  id: {user_id}\n",
            "  privateKey: \"{user_key}\"\n",
            "  name: Alice\n",
            "  email: alice@example.com\n",
            "  bio: Boson user\n",
            "  passphrase: secret\n",
            "device:\n",
            "  privateKey: \"{device_key}\"\n",
            "  name: Laptop\n",
            "  app: Boson\n",
        ),
        url = url,
        node_id = node_id,
        user_id = user_id,
        user_key = user_key,
        device_key = device_key,
    )
}

fn assert_loaded(
    options: &DirectorOptions,
    url: &str,
    node_id: &Id,
    user_key: &KeyPair,
    device_key: &KeyPair,
) {
    assert_eq!(options.director_url().as_str(), url);
    assert_eq!(options.node_id(), Some(node_id));
    assert_eq!(options.user_id(), Some(&Id::from(user_key.public_key())));
    assert_eq!(
        options.user_key().unwrap().private_key(),
        user_key.private_key()
    );
    assert_eq!(
        options.device_key().unwrap().private_key(),
        device_key.private_key()
    );
    assert!(options.insecure());
    assert_eq!(options.registration().name(), Some("Alice"));
    assert_eq!(
        options.registration().email(),
        Some("alice@example.com")
    );
    assert_eq!(options.registration().bio(), Some("Boson user"));
    assert_eq!(options.registration().passphrase(), Some("secret"));
    assert_eq!(options.registration().device_name(), Some("Laptop"));
    assert_eq!(options.registration().app_name(), Some("Boson"));
}

#[test]
fn test_from_url_options() {
    let options = DirectorOptions::from_url("https://director.example").unwrap();

    assert_eq!(
        options.director_url().as_str(),
        "https://director.example/"
    );
    assert_eq!(options.node_id(), None);
    assert_eq!(options.user_id(), None);
    assert!(options.user_key().is_none());
    assert!(options.device_key().is_none());
    assert!(!options.insecure());
    assert_eq!(options.registration().name(), None);
    assert_eq!(options.registration().email(), None);
    assert_eq!(options.registration().bio(), None);
    assert_eq!(options.registration().passphrase(), None);
    assert_eq!(options.registration().device_name(), None);
    assert_eq!(options.registration().app_name(), None);
}

#[test]
fn test_parse_options() {
    let node_id = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let content = yaml(
        "https://director.example",
        node_id,
        Id::from(user_key.public_key()),
        user_key.private_key(),
        device_key.private_key(),
    );

    let parsed = DirectorOptions::parse(&content).unwrap();
    let read = DirectorOptions::read(&content).unwrap();

    assert_loaded(
        &parsed,
        "https://director.example/",
        &node_id,
        &user_key,
        &device_key,
    );
    assert_loaded(
        &read,
        "https://director.example/",
        &node_id,
        &user_key,
        &device_key,
    );
}

#[test]
fn test_options_with_funs() {
    let node_id = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    let options = DirectorOptions::from_url("https://director.example")
        .unwrap()
        .with_director_url("http://127.0.0.1:9000")
        .unwrap()
        .with_node_id(node_id)
        .with_user_key(user_key.clone())
        .with_device_key(device_key.clone())
        .with_user_name("Bob")
        .with_user_email("bob@example.com")
        .with_user_bio("Director user")
        .with_user_passphrase("passphrase")
        .with_initial_device("Desktop", "Boson Director")
        .with_insecure(true);

    assert_eq!(
        options.director_url().as_str(),
        "http://127.0.0.1:9000/"
    );
    assert_eq!(options.node_id(), Some(&node_id));
    assert_eq!(options.user_id(), Some(&Id::from(user_key.public_key())));
    assert_eq!(
        options.user_key().unwrap().private_key(),
        user_key.private_key()
    );
    assert_eq!(
        options.device_key().unwrap().private_key(),
        device_key.private_key()
    );
    assert!(options.insecure());
    assert_eq!(options.registration().name(), Some("Bob"));
    assert_eq!(options.registration().email(), Some("bob@example.com"));
    assert_eq!(options.registration().bio(), Some("Director user"));
    assert_eq!(
        options.registration().passphrase(),
        Some("passphrase")
    );
    assert_eq!(options.registration().device_name(), Some("Desktop"));
    assert_eq!(
        options.registration().app_name(),
        Some("Boson Director")
    );
    options.check_valid().unwrap();
}

#[test]
fn test_load_then_overridden() {
    let loaded_node_id = Id::random();
    let loaded_user_key = KeyPair::random();
    let loaded_device_key = KeyPair::random();
    let config = ConfigFile::new(&yaml(
        "https://director.example",
        loaded_node_id,
        Id::from(loaded_user_key.public_key()),
        loaded_user_key.private_key(),
        loaded_device_key.private_key(),
    ));
    let replacement_node_id = Id::random();
    let replacement_user_key = KeyPair::random();
    let replacement_device_key = KeyPair::random();

    let options = DirectorOptions::load(config.path())
        .unwrap()
        .with_director_url("http://127.0.0.1:9001")
        .unwrap()
        .with_node_id(replacement_node_id)
        .with_user_key(replacement_user_key.clone())
        .with_device_key(replacement_device_key.clone())
        .with_user_name("Carol")
        .with_user_email("carol@example.com")
        .with_user_bio("Replacement user")
        .with_user_passphrase("replacement")
        .with_initial_device("Phone", "Boson Mobile")
        .with_insecure(false);

    assert_eq!(
        options.director_url().as_str(),
        "http://127.0.0.1:9001/"
    );
    assert_eq!(options.node_id(), Some(&replacement_node_id));
    assert_eq!(
        options.user_id(),
        Some(&Id::from(replacement_user_key.public_key()))
    );
    assert_eq!(
        options.user_key().unwrap().private_key(),
        replacement_user_key.private_key()
    );
    assert_eq!(
        options.device_key().unwrap().private_key(),
        replacement_device_key.private_key()
    );
    assert!(!options.insecure());
    assert_eq!(options.registration().name(), Some("Carol"));
    assert_eq!(
        options.registration().email(),
        Some("carol@example.com")
    );
    assert_eq!(options.registration().bio(), Some("Replacement user"));
    assert_eq!(
        options.registration().passphrase(),
        Some("replacement")
    );
    assert_eq!(options.registration().device_name(), Some("Phone"));
    assert_eq!(options.registration().app_name(), Some("Boson Mobile"));
}

#[test]
#[serial]
fn test_load_and_expand_environment_variables() {
    let node_id = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let values = [
        (
            "DIRECTOR_OPTIONS_TEST_URL",
            "https://director.example".to_string(),
        ),
        ("DIRECTOR_OPTIONS_TEST_NODE_ID", node_id.to_string()),
        (
            "DIRECTOR_OPTIONS_TEST_USER_ID",
            Id::from(user_key.public_key()).to_string(),
        ),
        (
            "DIRECTOR_OPTIONS_TEST_USER_KEY",
            user_key.private_key().to_string(),
        ),
        (
            "DIRECTOR_OPTIONS_TEST_DEVICE_KEY",
            device_key.private_key().to_string(),
        ),
    ];
    for (name, value) in &values {
        unsafe {
            env::set_var(name, value);
        }
    }
    let config = ConfigFile::new(&yaml(
        "${DIRECTOR_OPTIONS_TEST_URL}",
        "${DIRECTOR_OPTIONS_TEST_NODE_ID}",
        "${DIRECTOR_OPTIONS_TEST_USER_ID}",
        "${DIRECTOR_OPTIONS_TEST_USER_KEY}",
        "${DIRECTOR_OPTIONS_TEST_DEVICE_KEY}",
    ));

    let options = DirectorOptions::load(config.path()).unwrap();

    assert_loaded(
        &options,
        "https://director.example/",
        &node_id,
        &user_key,
        &device_key,
    );
    for (name, _) in &values {
        unsafe {
            env::remove_var(name);
        }
    }
}

#[test]
fn test_parse_rejects_mismatched_user_identity() {
    let content = yaml(
        "https://director.example",
        Id::random(),
        Id::random(),
        KeyPair::random().private_key(),
        KeyPair::random().private_key(),
    );

    let error = DirectorOptions::parse(content).unwrap_err();

    assert!(error.to_string().contains("does not match"));
}
