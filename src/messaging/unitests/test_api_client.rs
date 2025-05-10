use crate::{
    Id,
    core::crypto_identity::CryptoIdentity,
    signature
};

use crate::messaging::{
    api_client::{Builder},
};

const PEERID: &str = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ";
const NODEID: &str = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
const BASE_URL: &str = "http://155.138.245.211:8882";

#[tokio::test]
async fn test_service_ids() {
    let peerid:Id = PEERID.try_into().unwrap();
    let nodeid:Id = NODEID.try_into().unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device  = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let client = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device)
        .build()
        .unwrap();

    let result = client.service_ids().await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data.0, peerid);
    assert_eq!(data.1, nodeid);
}

#[tokio::test]
async fn test_register_user_and_device() {
    let peerid:Id = PEERID.try_into().unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device  = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let mut client = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device)
        .build()
        .unwrap();

    let result = client.register_user_and_device("password", "Alice", "Example", "Example").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_register_device_with_user() {
    let peerid:Id = PEERID.try_into().unwrap();
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

    let result = client1.register_user_and_device("password", "Alice", "Example", "Example").await;
    assert!(result.is_ok());

    let mut client2 = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device2)
        .build()
        .unwrap();

    let result = client2.register_device_with_user("password", "Alice", "Example").await;
    assert!(result.is_ok());
}

#[ignore]
#[tokio::test]
async fn test_regsister_device_request() {
    let peerid:Id = PEERID.try_into().unwrap();
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

    let result = client1.register_user_and_device("password", "Alice", "Example", "Example").await;
    assert!(result.is_ok());

    let mut client2 = Builder::new()
        .with_base_url(BASE_URL)
        .with_home_peerid(&peerid)
        .with_user_identity(&user)
        .with_device_identity(&device2)
        .build()
        .unwrap();

    let result = client2.register_device_request("Alice", "Example").await;
    assert!(result.is_ok());

    let registration_id = result.unwrap();
    let result = client1.finish_register_device_request(&registration_id, 0).await;
    assert!(result.is_ok());
}
