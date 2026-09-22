#[cfg(test)]
mod tests {
    use std::time::Duration;
    use boson::messaging::{Client, Options};

    #[test]
    fn test_client() {
        use boson::signature::PrivateKey;
        let res = PrivateKey::try_from("4WF77gvegeWyeGProxCxX2V1o996vneixdnewuE2XUpg");
        assert!(res.is_err(), "32-byte UserId must not be accepted as a 64-byte PrivateKey");
    }

    #[tokio::test]
    async fn test_messaging_client_info_fields() {
        use boson::messaging::MessagingClient;

        let options = Options::load("apps/chat/bob.yaml").unwrap();
        let client = Client::new(options);

        assert!(!client.user_id().to_string().is_empty());
        assert!(!client.device_id().to_string().is_empty());
        assert!(!client.service_peer_id().to_string().is_empty());
        assert!(client.service_endpoint().is_some());
        assert_eq!(client.connection_status(), "Disconnected");

        // Verify trait object access
        let trait_client: &dyn MessagingClient = &client;
        assert_eq!(trait_client.user_id(), client.user_id());
        assert_eq!(trait_client.device_id(), client.device_id());
        assert_eq!(trait_client.service_peer_id(), client.service_peer_id());
        assert_eq!(trait_client.service_endpoint(), client.service_endpoint());
        assert_eq!(trait_client.director_node_id(), client.director_node_id());
        assert_eq!(trait_client.director_endpoint(), client.director_endpoint());
        assert_eq!(trait_client.connection_status(), "Disconnected");
    }

    #[tokio::test]
    async fn test_client_friend_api_signatures() {
        use boson::Id;
        let options = Options::load("apps/chat/bob.yaml").unwrap();
        let client = Client::new(options);

        let target_id = Id::random();
        assert!(client.friend_request(target_id, Some("Hello".into())).await.is_ok());
        let req = client.get_friend_request(&target_id).await.unwrap();
        assert!(req.is_some());
        assert_eq!(req.unwrap().hello(), Some("Hello"));

        let requests = client.get_friend_requests().await.unwrap();
        assert_eq!(requests.len(), 1);

        assert!(client.remove_contact(&target_id).await.is_ok());
    }

    #[tokio::test]
    async fn test_verticle_connects_successfully() {
        let options = Options::load("apps/chat/bob.yaml").unwrap();
        let client = Client::new(options);
        
        let start_res = client.start().await;
        assert!(start_res.is_ok(), "Client start should succeed");

        // Give eventloop time to connect and subscribe
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if client.is_connected() && client.is_ready() {
                break;
            }
        }

        println!("Connected: {}, Ready: {}", client.is_connected(), client.is_ready());
        assert!(client.is_connected(), "Client should be connected");
        assert!(client.is_ready(), "Client should be ready");

        let stop_res = client.stop().await;
        assert!(stop_res.is_ok(), "Client stop should succeed");
    }
}
