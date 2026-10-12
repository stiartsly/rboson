#[cfg(test)]
mod tests {
    use std::time::Duration;
    use serial_test::serial;
    use tokio::time;

    use boson::{Id, signature::KeyPair};
    use boson::messaging::{Client, Message, MessageListener, Options};
    use boson::messaging::ContactType;

    const RECIPIENT_YAML: &str = "tests/apitests/messaging/recipient.yaml";
    const SENDER_YAML: &str = "tests/apitests/messaging/sender.yaml";

    async fn wait_until_ready(client: &Client) {
        time::timeout(Duration::from_secs(10), async {
            while !client.is_connected() || !client.is_ready() {
                time::sleep(Duration::from_millis(50)).await;
            }
        }).await
        .expect("Client should connect and subscribe within 10 seconds");
    }

    async fn wait_for_friend_request(
        client: &Client,
        user_id: boson::Id,
        created_at: std::time::SystemTime,
        accepted: bool,
    ) -> boson::messaging::FriendRequest {
        time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(request) = client.get_friend_request(user_id).await.unwrap() {
                    if request.created_at() == created_at && request.is_accepted() == accepted {
                        return request;
                    }
                }
                time::sleep(Duration::from_millis(50)).await;
            }
        }).await
        .expect("The matching friend request should arrive within 15 seconds")
    }

    struct MessageCapture {
        received: tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>,
    }

    impl MessageListener for MessageCapture {
        fn on_message(&self, message: &dyn Message) {
            let body = message
                .payload_as_content()
                .map(|content| content.body().to_vec());
            let _ = self.received.send(body);
        }
    }

    fn with_message_listener(
        options: Options,
        listener: impl MessageListener + 'static,
    ) -> Options {
        let mut builder = Options::builder();
        builder.with_peer_id(*options.peer_id());
        builder.with_peer_endpoint(options.peer_endpoint().as_str()).unwrap();
        builder.with_user_keypair(options.user_key().clone());
        builder.with_device_keypair(options.device_key().clone());
        builder.with_data_dir(options.data_dir());
        builder.with_database(options.database_uri(), options.database_pool_size()).unwrap();
        builder.with_database_schema(options.database_schema());
        builder.with_log_level(options.log_level());
        builder.with_log_console(options.log_console());
        if let Some(log_file) = options.log_file() {
            builder.with_log_file(log_file);
        }
        builder.with_message_listener(listener);
        builder.build().unwrap()
    }

    #[tokio::test]
    #[serial]
    async fn test_client_connection() {
        let options = Options::load(RECIPIENT_YAML).unwrap();
        let client = Client::new(options);

        let rc = client.start().await;
        assert!(rc.is_ok());
        wait_until_ready(&client).await;

        assert!(client.is_connected());
        assert!(client.is_ready());

        let rc = client.stop().await;
        assert!(rc.is_ok());

        time::sleep(Duration::from_secs(5)).await;

        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());

        let rc = client.start().await;
        assert!(rc.is_ok());
        wait_until_ready(&client).await;

        let rc = client.stop().await;
        assert!(rc.is_ok());

        time::sleep(Duration::from_secs(5)).await;

        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());

        let rc = client.stop().await;
        assert!(rc.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_friend_request() {
        let options = Options::load(SENDER_YAML).unwrap();
        let client = Client::new(options);

        let rc = client.start().await;
        assert!(rc.is_ok());

        wait_until_ready(&client).await;
        assert!(client.is_connected());
        assert!(client.is_ready());

        let target_id = Id::from(KeyPair::random().public_key());
        let rc = client.friend_request(target_id, Some("Hello")).await;
        assert!(rc.is_ok());

        let rc = client.get_friend_request(target_id).await;
        assert!(rc.is_ok());

        let request = rc.unwrap();
        assert!(request.is_some());
        assert_eq!(request.unwrap().hello(), Some("Hello"));

        let rc = client.get_friend_requests().await;
        assert!(rc.is_ok());

        let requests = rc.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(!requests.is_empty());
        assert!(requests.iter().any(|request|
            request.user_id() == &target_id && request.hello() == Some("Hello"))
        );

        let rc = client.remove_friend_request(target_id).await;
        assert!(rc.is_ok());
        let rc = client.get_friend_request(target_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        let rc = client.stop().await;
        assert!(rc.is_ok());

        time::sleep(Duration::from_secs(5)).await;

        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());
    }

    #[tokio::test]
    async fn test_client_start_failure() {
        let peerid = Id::from(KeyPair::random().public_key());
        let user_key = KeyPair::random();
        let dev_key = KeyPair::random();

        let mut builder = Options::builder();
        builder.with_peer_id(peerid);
        builder.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
        builder.with_user_keypair(user_key);
        builder.with_device_keypair(dev_key);
        builder.with_data_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));

        let rc = builder.build();
        assert!(rc.is_ok());

        let options = rc.unwrap();
        let client = Client::new(options);

        for _ in 0..2 {
            assert!(client.start().await.is_err(), "An existing file cannot be used as the data directory");
            assert!(!client.is_running());
            assert!(!client.is_connected());
            assert!(!client.is_ready());
        }
        assert!(client.stop().await.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_accept_friend_request() {
        let rc = Options::load(SENDER_YAML);
        assert!(rc.is_ok());
        let sender = Client::new(rc.unwrap());
        let sender_id = sender.user_id().clone();

        let rc = Options::load(RECIPIENT_YAML);
        assert!(rc.is_ok());
        let recipient = Client::new(rc.unwrap());
        let recipient_id = recipient.user_id().clone();

        assert_ne!(sender_id, recipient_id);
        let (rc1, rc2) = tokio::join!(sender.start(), recipient.start());
        assert!(rc1.is_ok() && rc2.is_ok());
        rc1.unwrap();
        rc2.unwrap();

        tokio::join!(
            wait_until_ready(&sender),
            wait_until_ready(&recipient)
        );

        // clear all friend requests and contacts before starting friend request
        sender.clear_contacts().await.unwrap();
        recipient.clear_contacts().await.unwrap();
        sender.clear_friend_requests().await.unwrap();
        recipient.clear_friend_requests().await.unwrap();

        let rc = recipient.accept_friend_request(sender_id).await;
        assert!(rc.is_err());

        let hello = Some("hello");
        let rc = sender.friend_request(recipient_id, hello).await;
        assert!(rc.is_ok());

        let rc = sender.get_friend_request(recipient_id).await;
        assert!(rc.is_ok());
        assert!(rc.as_ref().unwrap().is_some());

        let outgoing = rc.unwrap().unwrap();
        assert_eq!(outgoing.user_id(), &recipient_id);
        assert_eq!(outgoing.initiator_id(), &sender_id);
        assert_eq!(outgoing.hello(), Some("hello"));
        assert!(!outgoing.is_accepted());
        assert!(outgoing.accepted_at().is_none());

        let rc = sender.accept_friend_request(recipient_id).await;
        assert!(rc.is_err());

        let incoming = wait_for_friend_request(
            &recipient,
            sender_id,
            outgoing.created_at(),
            false
        ).await;
        assert_eq!(incoming.initiator_id(), &sender_id);
        assert_eq!(incoming.hello(), Some("hello"));

        let rc = recipient.accept_friend_request(sender_id).await;
        assert!(rc.is_ok());

        let accepted = wait_for_friend_request(
            &sender,
            recipient_id,
            outgoing.created_at(),
            true
        ).await;
        let rc = recipient.get_friend_request(sender_id).await;
        assert!(rc.is_ok());

        let local = rc.unwrap().unwrap();
        assert!(local.is_accepted());
        assert_eq!(accepted.accepted_at(), local.accepted_at());
        assert!(accepted.accepted_at().unwrap() >= accepted.created_at());

        let rc = recipient.accept_friend_request(sender_id).await;
        assert!(rc.is_err());
        let rc = sender.accept_friend_request(recipient_id).await;
        assert!(rc.is_err());

        let rc = sender.get_contact(&recipient_id).await;
        assert!(rc.is_ok());
        assert!(rc.as_ref().unwrap().is_some());

        let contact = rc.unwrap().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert_eq!(contact.revision(), 1);

        // check the contacts for sender.
        let rc = sender.remove_friend_request(recipient_id).await;
        assert!(rc.is_ok());
        let rc = sender.get_friend_request(recipient_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());
        let rc = sender.get_contact(&recipient_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_some());
        let rc = sender.remove_contact(&recipient_id).await;
        assert!(rc.is_ok());
        let rc = sender.get_contact(&recipient_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        // check contacts for recipient.
        let rc = recipient.get_contact(&sender_id).await;
        assert!(rc.is_ok());
        assert!(rc.as_ref().unwrap().is_some());

        let contact = rc.unwrap().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert_eq!(contact.revision(), 1);

        let rc = recipient.remove_friend_request(sender_id).await;
        assert!(rc.is_ok());
        let rc = recipient.get_friend_request(sender_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());
        let rc = recipient.get_contact(&sender_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_some());
        let rc = recipient.remove_contact(&sender_id).await;
        assert!(rc.is_ok());
        let rc = recipient.get_contact(&sender_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        let (rc1, rc2) = tokio::join!(
            sender.stop(),
            recipient.stop(),
        );
        assert!(rc1.is_ok());
        assert!(rc2.is_ok());

        time::sleep(Duration::from_secs(5)).await;

        assert!(!sender.is_running());
        assert!(!sender.is_connected());
        assert!(!sender.is_ready());

        assert!(!recipient.is_running());
        assert!(!recipient.is_connected());
        assert!(!recipient.is_ready());
    }

    #[tokio::test]
    #[serial]
    async fn test_message() {
        let options = Options::load(SENDER_YAML).unwrap();
        let sender = Client::new(options);
        let sender_id = sender.user_id().clone();

        let options = Options::load(RECIPIENT_YAML).unwrap();
        let (received_tx, mut received_rx) = tokio::sync::mpsc::unbounded_channel();
        let recipient = Client::new(with_message_listener(
            options,
            MessageCapture { received: received_tx },
        ));
        let recipient_id = *recipient.user_id();

        assert_ne!(sender_id, recipient_id);
        let (sender_start, recipient_start) = tokio::join!(
            sender.start(),
            recipient.start()
        );
        assert!(sender_start.is_ok());
        assert!(recipient_start.is_ok());

        tokio::join!(
            wait_until_ready(&sender),
            wait_until_ready(&recipient)
        );

        sender.clear_contacts().await.unwrap();
        recipient.clear_contacts().await.unwrap();
        sender.clear_friend_requests().await.unwrap();
        recipient.clear_friend_requests().await.unwrap();

        sender.friend_request(
            recipient_id,
            Some("message test")
        ).await.unwrap();
        let request = wait_for_friend_request(
            &recipient,
            sender_id,
            sender.get_friend_request(recipient_id).await.unwrap().unwrap().created_at(),
            false,
        ).await;
        recipient.accept_friend_request(sender_id).await.unwrap();
        wait_for_friend_request(
            &sender,
            recipient_id,
            request.created_at(),
            true,
        ).await;

        let body = "message delivery verification";
        let rc = sender.message(Some(recipient_id)).text_body(body).send().await;
        assert!(rc.is_ok());

        let sent = rc.unwrap();
        assert_eq!(sent.from(), Some(&sender_id));
        assert_eq!(sent.recipient(), &recipient_id);
        assert_eq!(
            sent.payload_as_content().and_then(|content| content.as_text()),
            Some(body)
        );
        let received = time::timeout(Duration::from_secs(15), received_rx.recv())
            .await
            .expect("The message should arrive within 15 seconds")
            .expect("The receiver should capture the inbound message");
        assert_eq!(received, Some(body.as_bytes().to_vec()));

        sender.clear_contacts().await.unwrap();
        recipient.clear_contacts().await.unwrap();
        sender.clear_friend_requests().await.unwrap();
        recipient.clear_friend_requests().await.unwrap();

        let (sender_stop, recipient_stop) = tokio::join!(
            sender.stop(),
            recipient.stop()
        );
        assert!(sender_stop.is_ok());
        assert!(recipient_stop.is_ok());
    }
}
