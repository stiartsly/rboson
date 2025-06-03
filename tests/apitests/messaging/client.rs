use boson::{
    signature,
    Id,
    messaging::MessagingClient,
    messaging::Client,
    messaging::ClientBuilder,
    messaging::ConnectionListener,
};


const PEERID: &str = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ";
const NODEID: &str = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
const BASE_URL: &str = "http://155.138.245.211:8882";

#[tokio::test]
async fn test_service_ids() {
    let url = url::Url::parse(BASE_URL).unwrap();
    let nodeid = Id::try_from(NODEID).unwrap();
    let peerid = Id::try_from(PEERID).unwrap();

    let result = Client::service_ids(&url).await;
    assert!(result.is_ok());

    let result = ClientBuilder::service_ids(&url).await;
    assert!(result.is_ok());

    let ids = result.unwrap();
    assert_eq!(ids.peerid(), &peerid);
    assert_eq!(ids.nodeid(), &nodeid);
}

#[ignore]
#[tokio::test]
async fn test_messaing_client() {
    let peerid = Id::try_from(PEERID).unwrap();
    let user_key = signature::KeyPair::random();
    let result = ClientBuilder::new()
        .with_user_key(&user_key)
        .with_peerid(&peerid)
        .with_device_name("test-Device")
        .with_app_name("test-App")
        .register_user_and_device("secret")
        .with_messaging_repository("test-repo")
        .with_connection_listener(Box::new({
            struct TestConnectionListener;
            impl ConnectionListener for TestConnectionListener {}
            TestConnectionListener {}
        }))
        .build()
        .await;

    assert!(result.is_ok());

    let client = result.unwrap();
    let userid = Id::from(user_key.to_public_key());

    assert_eq!(client.userid(), &userid);
}
