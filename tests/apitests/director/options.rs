use boson::{
    director::{Options, UserRegistration},
    signature::KeyPair,
    Id,
};

fn assert_loaded(
    options: &Options,
    url: &str,
    node_id: &Id,
    user_key: &KeyPair,
    device_key: &KeyPair,
) {
    assert_eq!(options.director_url().as_str(), url);
    assert_eq!(options.node_id(), Some(node_id));
    assert_eq!(options.user_id(), Some(&Id::from(user_key.public_key())));
    assert_eq!(options.user_private_key().unwrap(), user_key.private_key());
    assert_eq!(
        options.device_private_key().unwrap(),
        device_key.private_key()
    );
    assert!(options.is_insecure());

    let registration = options.registration().unwrap();
    assert_eq!(registration.name(), Some("Alice"));
    assert_eq!(registration.email(), Some("alice@example.com"));
    assert_eq!(registration.bio(), Some("Boson user"));
    assert_eq!(registration.passphrase(), Some("secret"));
    assert_eq!(registration.device_name(), Some("Laptop"));
    assert_eq!(registration.app_name(), Some("Boson"));
}

#[test]
fn test_from_url_options() {
    let options = Options::new("https://director.example").unwrap();

    assert_eq!(options.director_url().as_str(), "https://director.example/");
    assert_eq!(options.node_id(), None);
    assert_eq!(options.user_id(), None);
    assert!(options.user_private_key().is_none());
    assert!(options.device_private_key().is_none());
    assert!(!options.is_insecure());
    assert!(options.registration().is_none());
    assert!(options.check_completeness().is_ok());
}

#[test]
fn test_options_with_builders() {
    let node_id = Id::random();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let registration = UserRegistration::new()
        .with_name("Alice")
        .with_email("alice@example.com")
        .with_bio("Boson user")
        .with_passphrase("secret")
        .with_initial_device("Laptop", "Boson");

    let options = Options::new("https://director.example")
        .unwrap()
        .with_node_id(node_id)
        .with_user_id(Id::from(user_key.public_key()))
        .with_user_private_key(user_key.private_key().clone())
        .with_device_private_key(device_key.private_key().clone())
        .with_registration(registration)
        .with_insecure(true);

    assert_loaded(
        &options,
        "https://director.example/",
        &node_id,
        &user_key,
        &device_key,
    );
    options.check_completeness().unwrap();
}

#[test]
fn test_user_id_builder_clears_user_private_key() {
    let user_key = KeyPair::random();
    let options = Options::new("https://director.example")
        .unwrap()
        .with_user_private_key(user_key.private_key().clone())
        .with_user_id(Id::random());

    assert!(options.user_private_key().is_none());
    assert!(options.device_private_key().is_none());
    assert!(options.check_completeness().is_err());
}

#[test]
fn test_check_completion_rejects_device_without_user_identity() {
    let options = Options::new("https://director.example")
        .unwrap()
        .with_device_private_key(KeyPair::random().private_key().clone());

    let error = options.check_completeness().unwrap_err();
    assert!(error.to_string().contains("device key"));
}

#[test]
fn test_check_completion_rejects_user_id_without_device() {
    let options = Options::new("https://director.example")
        .unwrap()
        .with_user_id(Id::random());

    let error = options.check_completeness().unwrap_err();
    assert!(error.to_string().contains("device key"));
}

#[test]
fn test_private_key_builders_update_identity_state() {
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let options = Options::new("https://director.example")
        .unwrap()
        .with_user_private_key(user_key.private_key().clone())
        .with_device_private_key(device_key.private_key().clone());

    /*assert_eq!(
        options.user_id(),
        None,
        "setting a private key does not infer a user ID"
    );*/
    assert_eq!(options.user_private_key(), Some(user_key.private_key()));
    assert_eq!(options.device_private_key(), Some(device_key.private_key()));
    assert!(options.check_completeness().is_ok());
}
