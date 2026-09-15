use crate::{
    director::{DirectorOptions, DirectorOptionsBuilder},
    errors::ArgumentError,
    signature::KeyPair,
    Id,
};
use std::fs;

#[test]
fn builder_installs_user_and_registration_information() {
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let user_id = Id::from(user_key.public_key());

    let options = DirectorOptions::builder("https://director.example")
        .unwrap()
        .with_user_key(user_key)
        .with_device_key(device_key)
        .with_user_name("Alice")
        .with_user_email("alice@example.com")
        .with_user_bio("Boson user")
        .with_user_passphrase("secret")
        .with_initial_device("Laptop", "Boson")
        .with_insecure(true)
        .build()
        .unwrap();

    assert_eq!(options.user_id(), Some(&user_id));
    assert!(options.user_key().is_some());
    assert!(options.device_key().is_some());
    assert!(options.insecure());
    assert_eq!(options.registration().name(), Some("Alice"));
    assert_eq!(options.registration().email(), Some("alice@example.com"));
    assert_eq!(options.registration().bio(), Some("Boson user"));
    assert_eq!(options.registration().passphrase(), Some("secret"));
    assert_eq!(options.registration().device_name(), Some("Laptop"));
    assert_eq!(options.registration().app_name(), Some("Boson"));
}

#[test]
fn builder_reads_user_information_from_yaml() {
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let user_id = Id::from(user_key.public_key());
    let yaml = format!(
        r#"
director:
  url: https://director.example
  nodeId: {}
  insecure: true
user:
  id: {}
  privateKey: "{}"
  name: Alice
  email: alice@example.com
  bio: Boson user
  passphrase: secret
device:
  privateKey: "{}"
  name: Laptop
  app: Boson
"#,
        Id::random(),
        user_id,
        user_key.private_key().to_hexstr(),
        device_key.private_key().to_hexstr(),
    );

    let options = DirectorOptionsBuilder::read_from(&yaml)
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(options.director_url().as_str(), "https://director.example/");
    assert_eq!(options.user_id(), Some(&user_id));
    assert!(options.user_key().is_some());
    assert!(options.device_key().is_some());
    assert!(options.insecure());
    assert_eq!(options.registration().name(), Some("Alice"));
    assert_eq!(options.registration().device_name(), Some("Laptop"));
}

#[test]
fn builder_loads_yaml_file() {
    let path = std::env::temp_dir().join(format!(
        "boson-director-options-{}-{}.yaml",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::write(
        &path,
        "director:\n  url: https://director.example\n  insecure: false\n",
    )
    .unwrap();

    let options = DirectorOptionsBuilder::load_from(&path)
        .unwrap()
        .build()
        .unwrap();
    fs::remove_file(&path).unwrap();

    assert_eq!(options.director_url().as_str(), "https://director.example/");
    assert!(options.user_id().is_none());
    assert!(!options.insecure());
}

#[test]
fn yaml_rejects_mismatched_user_id_and_private_key() {
    let yaml = format!(
        r#"
director:
  url: https://director.example
user:
  id: {}
  privateKey: "{}"
"#,
        Id::random(),
        KeyPair::random().private_key().to_hexstr(),
    );

    let error = DirectorOptionsBuilder::read_from(&yaml).unwrap_err();
    assert!(error.downcast_ref::<ArgumentError>().is_some());
}
