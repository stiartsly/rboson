use crate::{
    Id,
    core::crypto_identity::CryptoIdentity,
    //Identity,
    //cryptobox::Nonce,
    signature
};

use crate::messaging::{
    api_client::APIClient
};

//use serde::{Serialize, Deserialize};

const PEERID: &str = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ";
const NODEID: &str = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
const BASE_URL: &str = "http://155.138.245.211:8882";

/*
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[allow(non_snake_case)]
struct RefreshAccessTokenReqData {
    userId  : Id,
    deviceId: Id,
    nonce   : Vec<u8>,
    userSig : Vec<u8>,
    deviceSig: Vec<u8>
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[allow(non_snake_case)]
struct GetAccessTokenRspData {
    token   : Option<String>,
}

#[tokio::test]
async fn test_refresh_access_token_data() {
    let user = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let nonce = Nonce::random();
    let data = RefreshAccessTokenReqData {
        userId  : user.id().clone(),
        deviceId: device.id().clone(),
        nonce   : nonce.as_bytes().to_vec(),
        userSig : user.sign_into(nonce.as_bytes()).unwrap(),
        deviceSig: device.sign_into(nonce.as_bytes()).unwrap()
    };

    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: RefreshAccessTokenReqData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(data.userId, deserialized.userId);
    assert_eq!(data.deviceId, deserialized.deviceId);
    assert_eq!(data.nonce, deserialized.nonce);
    assert_eq!(data.userSig, deserialized.userSig);
    assert_eq!(data.deviceSig, deserialized.deviceSig);
    assert_eq!(data, deserialized);

    let data = GetAccessTokenRspData {
        token: Some("bear token".to_string())
    };

    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: GetAccessTokenRspData = serde_json::from_str(&serialized).unwrap();
    assert!(data.token.is_some());
    assert!(deserialized.token.is_some());
    assert_eq!(data.token.as_ref().unwrap(), deserialized.token.as_ref().unwrap());
}


#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[allow(non_snake_case)]
struct RegisterUserAndDeviceReqData {
    userId  : Id,
    userName: String,
    passphrase  : String,
    deviceId: Id,
    deviceName  : String,
    appName : String,
    nonce   : Vec<u8>,
    userSig : Vec<u8>,
    deviceSig   : Vec<u8>,
    profileSig  : Vec<u8>
}

#[tokio::test]
async fn test_register_user_and_device_data() {
    let user = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let profile_digest = "profile digest".as_bytes().to_vec();
    let nonce = Nonce::random();
    let data = RegisterUserAndDeviceReqData {
        userId  : user.id().clone(),
        userName: "Alice".to_string(),
        passphrase  : "password".to_string(),
        deviceId: device.id().clone(),
        deviceName  : "Example".to_string(),
        appName : "Example".to_string(),
        nonce   : nonce.as_bytes().to_vec(),
        userSig : user.sign_into(nonce.as_bytes()).unwrap(),
        deviceSig: device.sign_into(nonce.as_bytes()).unwrap(),
        profileSig: user.sign_into(&profile_digest).unwrap()
    };

    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: RegisterUserAndDeviceReqData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(data.userId, deserialized.userId);
    assert_eq!(data.userName, deserialized.userName);
    assert_eq!(data.passphrase, deserialized.passphrase);
    assert_eq!(data.deviceId, deserialized.deviceId);
    assert_eq!(data.deviceName, deserialized.deviceName);
    assert_eq!(data.nonce, deserialized.nonce);
    assert_eq!(data.userSig, deserialized.userSig);
    assert_eq!(data.deviceSig, deserialized.deviceSig);
    assert_eq!(data.profileSig, deserialized.profileSig);
    assert_eq!(data, deserialized);
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[allow(non_snake_case)]
struct RegisterDeviceWithUserReqData {
    userId: Id,
    passphrase: String,
    deviceId: Id,
    deviceName: String,
    appName: String,
    nonce: Vec<u8>,
    userSig: Vec<u8>,
    deviceSig: Vec<u8>
}

#[tokio::test]
async fn test_register_device_with_user_data() {
    let user = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let nonce = Nonce::random();
    let data = RegisterDeviceWithUserReqData {
        userId  : user.id().clone(),
        passphrase  : "password".to_string(),
        deviceId: device.id().clone(),
        deviceName  : "Example".to_string(),
        appName : "Example".to_string(),
        nonce   : nonce.as_bytes().to_vec(),
        userSig : user.sign_into(nonce.as_bytes()).unwrap(),
        deviceSig: device.sign_into(nonce.as_bytes()).unwrap(),
    };

    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: RegisterDeviceWithUserReqData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(data.userId, deserialized.userId);
    assert_eq!(data.passphrase, deserialized.passphrase);
    assert_eq!(data.deviceId, deserialized.deviceId);
    assert_eq!(data.deviceName, deserialized.deviceName);
    assert_eq!(data.nonce, deserialized.nonce);
    assert_eq!(data.userSig, deserialized.userSig);
    assert_eq!(data.deviceSig, deserialized.deviceSig);
    assert_eq!(data, deserialized);
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[allow(non_snake_case)]
struct ServiceIdsRspData {
    peerId: Option<Id>,
    nodeId: Option<Id>
}

#[tokio::test]
async fn test_service_ids_data() {
    let data = ServiceIdsRspData {
        peerId: Some(Id::random()),
        nodeId: Some(Id::random()),
    };

    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: ServiceIdsRspData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(data.peerId, deserialized.peerId);
    assert_eq!(data.nodeId, deserialized.nodeId);
    assert_eq!(data, deserialized);
}
*/

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

    let token = result.unwrap();
    println!("result: {}", token);
}
