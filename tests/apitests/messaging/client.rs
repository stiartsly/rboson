#[cfg(test)]
mod tests {
    use std::time::Duration;
    use boson::messaging::{Client, Options};

    #[tokio::test]
    async fn test_client_friend_request() {
        use boson::{Id, signature::KeyPair};
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

        assert!(client.is_connected() && client.is_ready(), "Client should be connected and ready");

        let target_id = Id::from(KeyPair::random().public_key());
        let rc = client.friend_request(target_id, Some("Hello".into())).await;
        assert!(rc.is_ok());

        let req = client.get_friend_request(target_id).await.unwrap();
        assert!(req.is_some());
        assert_eq!(req.unwrap().hello(), Some("Hello"));

        let requests = client.get_friend_requests().await.unwrap();
        assert!(requests.iter().any(|request| request.user_id() == &target_id && request.hello() == Some("Hello")));

        assert!(client.remove_friend_request(target_id).await.is_ok());
        assert!(client.get_friend_request(target_id).await.unwrap().is_none());
        assert!(client.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_client_connection() {
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
