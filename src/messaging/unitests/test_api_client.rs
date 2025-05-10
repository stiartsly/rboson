use crate::{
    Id,
    core::crypto_identity::CryptoIdentity,
    signature
};

use crate::messaging::{
    api_client::APIClient
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

    let mut client = APIClient::new(&peerid, BASE_URL).unwrap();
    client.with_user_identity(&user);
    client.with_device_identity(&device);

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

    let mut client = APIClient::new(&peerid, BASE_URL).unwrap();
    client.with_user_identity(&user);
    client.with_device_identity(&device);

    let result = client.register_user_and_device("password", "Alice", "Example", "Example").await;
    assert!(result.is_ok());
}

#[ignore]
#[tokio::test]
async fn test_register_device_with_user() {
    let peerid:Id = PEERID.try_into().unwrap();
    let user    = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device  = CryptoIdentity::from_keypair(signature::KeyPair::random());

    let mut client = APIClient::new(&peerid, BASE_URL).unwrap();
    client.with_user_identity(&user);
    client.with_device_identity(&device);

    let result = client.register_user_and_device("password", "Alice", "Example", "Example").await;
    assert!(result.is_ok());

    let result = client.register_device_with_user("password", "Alice", "Example").await;
    println!("Result: {:?}", result);
    assert!(result.is_ok());
}
