use crate::{
    Id,
    MessagingClient,
    MessagingClientBuilder,
};

const PEERID: &str = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ";
const NODEID: &str = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
const BASE_URL: &str = "http://155.138.245.211:8882";

#[tokio::test]
async fn test_service_ids() {
    let url = url::Url::parse(BASE_URL).unwrap();
    let nodeid = Id::try_from(NODEID).unwrap();
    let peerid = Id::try_from(PEERID).unwrap();

    let result = MessagingClient::service_ids(&url).await;
    assert!(result.is_ok());

    let result = MessagingClientBuilder::service_ids(&url).await;
    assert!(result.is_ok());

    let ids = result.unwrap();
    assert_eq!(ids.peerid(), &peerid);
    assert_eq!(ids.nodeid(), &nodeid);
}
