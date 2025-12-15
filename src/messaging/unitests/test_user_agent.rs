use crate::{
    signature,
    core::CryptoIdentity,
    messaging::{
        UserAgentCaps,
        UserAgent
    }
};

#[tokio::test]
async fn test_user_agent() {
    let mut agent = UserAgent::new(None);
    assert!(agent.user().is_none());
    assert!(agent.device().is_none());

    let user_identity = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device_identity = CryptoIdentity::from_keypair(signature::KeyPair::random());

    _ = agent.set_user(user_identity.clone(), "Alice".into());
    _ = agent.set_device(device_identity.clone(), "Example".into(), "Example");

    assert!(agent.user().is_some());
    assert!(agent.device().is_some());

    let user = agent.user().unwrap();
    let device = agent.device().unwrap();

    assert_eq!(user.id(), user_identity.id());
    assert_eq!(device.id(), Some(device_identity.id()));
    assert!(true);
}
