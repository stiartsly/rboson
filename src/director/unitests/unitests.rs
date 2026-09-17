use crate::director::{
    base64url, sign_nonce, Client, Options, ProfileUpdate, UserRegistration,
};
use crate::{errors::ArgumentError, signature::KeyPair, Id};

#[test]
fn builder_rejects_incomplete_device_identity() {
    let device = KeyPair::random();
    let options = Options::new("https://director.example")
        .unwrap()
        .with_device_private_key(device.private_key().clone());
    let result = options.check_completion();

    let error = match result {
        Ok(_) => panic!("device-only configuration must fail"),
        Err(error) => error,
    };
    assert!(error.downcast_ref::<ArgumentError>().is_some());
}

#[test]
fn builder_derives_user_and_device_ids() {
    let user = KeyPair::random();
    let device = KeyPair::random();
    let expected_user = Id::from(user.public_key());
    let expected_device = Id::from(device.public_key());
    let options = Options::new("https://director.example/prefix/")
        .unwrap()
        .with_user_id(expected_user.clone())
        .with_user_private_key(user.private_key().clone())
        .with_device_private_key(device.private_key().clone());
    let client = Client::new(options).unwrap();

    assert_eq!(client.user_id(), Some(&expected_user));
    assert_eq!(client.device_id(), Some(&expected_device));
    assert!(!client.is_closed());
    client.close();
    assert!(client.is_closed());
}

#[test]
fn profile_update_preserves_clear_operations() {
    let update = ProfileUpdate::new().name(Some("Alice".into())).email(None);
    let fields = update.fields();

    assert_eq!(fields["name"], "Alice");
    assert!(fields["email"].is_null());
    assert!(!update.is_empty());
}

#[test]
fn user_registration_tracks_initial_device() {
    let registration = UserRegistration::new()
        .with_name("Alice")
        .with_initial_device("Laptop", "Boson");

    assert_eq!(registration.name(), Some("Alice"));
    assert_eq!(registration.device_name(), Some("Laptop"));
    assert_eq!(registration.app_name(), Some("Boson"));
}

#[test]
fn device_auth_signature_uses_unpadded_base64url() {
    let key = KeyPair::random();
    let nonce = [42; 32];
    let signature = sign_nonce(&key, &nonce).unwrap();
    use base64::Engine;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&signature)
        .unwrap();

    assert!(!signature.contains('='));
    assert!(key.public_key().verify(&nonce, &decoded).unwrap());
    assert_eq!(base64url(&[0xff, 0xee]), "_-4");
}
