use crate::{
    Id,
    Identity,
    core::crypto_identity::CryptoIdentity,
    signature
};

use crate::messaging::{
    api_client::{APIClient, Builder},
};

const PEERID: &str = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ";
const NODEID: &str = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
const BASE_URL: &str = "http://155.138.245.211:8882";

#[tokio::test]
async fn test_service_ids() {
    let nodeid = Id::try_from(NODEID).unwrap();
    let peerid = Id::try_from(PEERID).unwrap();

    let url = url::Url::parse(BASE_URL).unwrap();
    let result = APIClient::service_ids(&url).await;

    assert!(result.is_ok());

    let ids = result.unwrap();
    assert_eq!(ids.peerid(), &peerid);
    assert_eq!(ids.nodeid(), &nodeid);
}

#[tokio::test]
async fn test_register_user_and_device() {
    let peerid  = Id::try_from(PEERID).unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device  = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let client = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device)
        .build();

    assert!(client.is_ok());

    let result = client.unwrap()
        .register_user_with_device("password", "Alice", "test-Device", "test-App")
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_register_device_with_user() {
    let peerid  = Id::try_from(PEERID).unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device1 = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device2 = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let client1 = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device1)
        .build();

    let result = client1.unwrap()
        .register_user_with_device("password", "Alice", "test-Device1", "test-App")
        .await;
    assert!(result.is_ok());

    let client2 = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device2)
        .build();

    let result = client2.unwrap()
        .register_new_device("password", "test-Device2", "test-App")
        .await;
    assert!(result.is_ok());
}

#[ignore]
#[tokio::test]
async fn test_regsister_device_request() {
    let peerid  = Id::try_from(PEERID).unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device1 = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device2 = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let mut client1 = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device1)
        .build()
        .unwrap();

    let result = client1.register_user_with_device(
        "password",
        "Alice",
        "test-Device1",
        "test-App"
    ).await;
    assert!(result.is_ok());

    let mut client2 = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device2)
        .build()
        .unwrap();

    let result = client2.register_device_request("test-Device2", "test-App").await;
    assert!(result.is_ok());

    let registration_id = result.unwrap();
    let result = client2.finish_register_device_request(&registration_id, 0).await;
    println!("result: {:?}", result);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_profile() {
    let peerid  = Id::try_from(PEERID).unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device  = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let mut client = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device)
        .build()
        .unwrap();

    let result = client.register_user_with_device(
        "password",
        "Alice",
        "test-Device1",
        "test-App"
    ).await;
    assert!(result.is_ok());

    // Get profile and check initial values
    let result = client.get_profile(user.id()).await;
    assert!(result.is_ok());
    let profile = result.unwrap();
    assert_eq!(profile.id(), user.id());
    assert_eq!(profile.home_peerid(), &peerid);
    assert_eq!(profile.name(), "Alice");
    assert_eq!(profile.notice(), None);
    assert!(profile.is_genuine());

    // Update profile
    let result = client.update_profile("Bob", false).await;
    assert!(result.is_ok());

    // Get profile and check updated values
    let result = client.get_profile(user.id()).await;
    assert!(result.is_ok());
    let profile = result.unwrap();
    assert_eq!(profile.id(), user.id());
    assert_eq!(profile.home_peerid(), &peerid);
    assert_eq!(profile.name(), "Bob");
    assert_eq!(profile.notice(), None);
    assert_eq!(profile.sig().len(), 64);
    assert!(profile.is_genuine());
}
