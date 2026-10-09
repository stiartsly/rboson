#[cfg(test)]
mod tests {
    use std::time::Duration;
    use boson::messaging::{Client, Options};
    use serial_test::serial;

    async fn wait_until_ready(client: &Client) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !client.is_connected() || !client.is_ready() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await.expect("Client should connect and subscribe within 10 seconds");
    }

    async fn wait_for_friend_request(
        client: &Client,
        user_id: boson::Id,
        created_at: std::time::SystemTime,
        accepted: bool,
    ) -> boson::messaging::FriendRequest {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(request) = client.get_friend_request(user_id).await.unwrap() {
                    if request.created_at() == created_at && request.is_accepted() == accepted {
                        return request;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await.expect("The matching friend request should arrive within 15 seconds")
    }

    async fn request_and_accept(sender_config: &str, recipient_config: &str, hello: Option<String>) {
        let sender = Client::new(Options::load(sender_config).unwrap());
        let recipient = Client::new(Options::load(recipient_config).unwrap());
        let sender_id = *sender.user_id();
        let recipient_id = *recipient.user_id();
        assert_ne!(sender_id, recipient_id);
        let (sender_start, recipient_start) = tokio::join!(sender.start(), recipient.start());
        sender_start.unwrap();
        recipient_start.unwrap();
        tokio::join!(wait_until_ready(&sender), wait_until_ready(&recipient));
        sender.clear_contacts().await.unwrap();
        recipient.clear_contacts().await.unwrap();
        sender.clear_friend_requests().await.unwrap();
        recipient.clear_friend_requests().await.unwrap();

        assert!(recipient.accept_friend_request(sender_id).await.is_err());
        sender.friend_request(recipient_id, hello.clone()).await.unwrap();
        let outgoing = sender.get_friend_request(recipient_id).await.unwrap().unwrap();
        assert_eq!(outgoing.user_id(), &recipient_id);
        assert_eq!(outgoing.initiator_id(), &sender_id);
        assert_eq!(outgoing.hello(), hello.as_deref());
        assert!(!outgoing.is_accepted());
        assert!(outgoing.accepted_at().is_none());
        assert!(sender.accept_friend_request(recipient_id).await.is_err());

        let incoming = wait_for_friend_request(&recipient, sender_id, outgoing.created_at(), false).await;
        assert_eq!(incoming.initiator_id(), &sender_id);
        assert_eq!(incoming.hello(), Some(hello.as_deref().unwrap_or_default()));
        recipient.accept_friend_request(sender_id).await.unwrap();
        let accepted = wait_for_friend_request(&sender, recipient_id, outgoing.created_at(), true).await;
        let local = recipient.get_friend_request(sender_id).await.unwrap().unwrap();
        assert!(local.is_accepted());
        assert_eq!(accepted.accepted_at(), local.accepted_at());
        assert!(accepted.accepted_at().unwrap() >= accepted.created_at());
        assert!(recipient.accept_friend_request(sender_id).await.is_err());
        assert!(sender.accept_friend_request(recipient_id).await.is_err());

        for (client, friend_id) in [(&sender, recipient_id), (&recipient, sender_id)] {
            let contact = client.get_contact(&friend_id).await.unwrap().unwrap();
            assert_eq!(contact.contact_type(), boson::messaging::ContactType::Friend);
            assert_eq!(contact.revision(), 1);
            client.remove_friend_request(friend_id).await.unwrap();
            assert!(client.get_friend_request(friend_id).await.unwrap().is_none());
            assert!(client.get_contact(&friend_id).await.unwrap().is_some());
            client.remove_contact(&friend_id).await.unwrap();
            assert!(client.get_contact(&friend_id).await.unwrap().is_none());
        }
        let (sender_stop, recipient_stop) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(sender.stop(), recipient.stop())
        }).await.unwrap();
        sender_stop.unwrap();
        recipient_stop.unwrap();
        assert!(!sender.is_running() && !sender.is_connected() && !sender.is_ready());
        assert!(!recipient.is_running() && !recipient.is_connected() && !recipient.is_ready());
    }

    #[tokio::test]
    #[serial]
    async fn test_client_accept_friend_request_bob_to_alice() {
        request_and_accept("apps/chat/bob.yaml", "apps/chat/alice.yaml", None).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_client_accept_friend_request_alice_to_bob() {
        request_and_accept("apps/chat/alice.yaml", "apps/chat/bob.yaml", Some("Hello Bob".into())).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_client_friend_request() {
        use boson::{Id, signature::KeyPair};
        let options = Options::load("apps/chat/bob.yaml").unwrap();
        let client = Client::new(options);

        let start_res = client.start().await;
        assert!(start_res.is_ok(), "Client start should succeed");

        wait_until_ready(&client).await;

        assert!(client.is_connected() && client.is_ready(), "Client should be connected and ready");

        let target_id = Id::from(KeyPair::random().public_key());
        let rc = client.friend_request(target_id, Some("Hello".into())).await;
        assert!(rc.is_ok(), "Friend request should succeed: {:?}", rc);

        let req = client.get_friend_request(target_id).await.unwrap();
        assert!(req.is_some());
        assert_eq!(req.unwrap().hello(), Some("Hello"));

        let requests = client.get_friend_requests().await.unwrap();
        assert!(requests.iter().any(|request| request.user_id() == &target_id && request.hello() == Some("Hello")));

        assert!(client.remove_friend_request(target_id).await.is_ok());
        assert!(client.get_friend_request(target_id).await.unwrap().is_none());
        assert!(tokio::time::timeout(Duration::from_secs(5), client.stop()).await.unwrap().is_ok());
        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());
    }

    #[tokio::test]
    #[serial]
    async fn test_client_connection() {
        let options = Options::load("apps/chat/bob.yaml").unwrap();
        let client = Client::new(options);

        let start_res = client.start().await;
        assert!(start_res.is_ok(), "Client start should succeed");

        wait_until_ready(&client).await;

        println!("Connected: {}, Ready: {}", client.is_connected(), client.is_ready());
        assert!(client.is_connected(), "Client should be connected");
        assert!(client.is_ready(), "Client should be ready");

        let stop_res = tokio::time::timeout(Duration::from_secs(5), client.stop()).await.unwrap();
        assert!(stop_res.is_ok(), "Client stop should succeed");
        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());

        assert!(client.start().await.is_ok(), "Client restart should succeed");
        wait_until_ready(&client).await;
        assert!(tokio::time::timeout(Duration::from_secs(5), client.stop()).await.unwrap().is_ok());
        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());
        assert!(client.stop().await.is_ok(), "Stopping an already stopped client should succeed");
    }

    #[tokio::test]
    async fn test_client_start_failure_can_be_retried() {
        use boson::{Id, signature::KeyPair};
        let mut options = Options::builder();
        options.with_peer_id(Id::from(KeyPair::random().public_key()));
        options.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
        options.with_user_keypair(KeyPair::random());
        options.with_device_keypair(KeyPair::random());
        options.with_data_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let client = Client::new(options.build().unwrap());

        for _ in 0..2 {
            assert!(client.start().await.is_err(), "An existing file cannot be used as the data directory");
            assert!(!client.is_running());
            assert!(!client.is_connected());
            assert!(!client.is_ready());
        }
        assert!(client.stop().await.is_ok());
    }
}
