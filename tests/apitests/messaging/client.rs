#[cfg(test)]
mod tests {
    use std::time::Duration;
    use serial_test::serial;
    use tokio::time;
    use tokio::sync::mpsc;

    use boson::{Id, signature::KeyPair};
    use boson::messaging::{Client, Message, MessageListener, Options, Permission};
    use boson::messaging::ContactType;

    const BOB_YAML: &str = "tests/apitests/messaging/bob.yaml";
    const ALICE_YAML: &str = "tests/apitests/messaging/alice.yaml";

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

    struct ChannelMessageCapture {
        received: tokio::sync::mpsc::UnboundedSender<(Id, Option<Id>, Id, Option<Vec<u8>>)>,
    }

    impl MessageListener for ChannelMessageCapture {
        fn on_message(&self, message: &dyn Message) {
            let body = message.payload_as_content().map(|content| content.body().to_vec());
            self.received.send((*message.id(), message.from().copied(), *message.recipient(), body))
                .expect("Channel testcase receiver should remain open");
        }
    }

    fn build_options_with_message_listener(
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
        let options = Options::load(ALICE_YAML).unwrap();
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
        let options = Options::load(BOB_YAML).unwrap();
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
    async fn test_get_sessions() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());
        let rc = client.start().await;
        assert!(rc.is_ok());

        wait_until_ready(&client).await;

        let rc = client.get_sessions().await;
        let sessions = rc.unwrap();

        let session_opt = sessions.iter()
            .find(|session| session.device_id() == client.device_id());

        assert!(session_opt.is_some());

        let current_session = session_opt.unwrap();
        assert!(current_session.is_online());
        assert!(current_session.last_active_ms() > 0);

        assert_eq!(
            sessions.iter().filter(|session| session.device_id() == client.device_id()).count(),
            1
        );

        assert!(client.stop().await.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_accept_friend_request() {
        let rc = Options::load(BOB_YAML);
        assert!(rc.is_ok());
        let sender = Client::new(rc.unwrap());
        let sender_id = sender.user_id().clone();

        let rc = Options::load(ALICE_YAML);
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
    async fn tests_add_friend() {
        let bob = Client::new(Options::load(BOB_YAML).unwrap());
        let alice = Client::new(Options::load(ALICE_YAML).unwrap());
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        assert_ne!(bob_id, alice_id);

        let (bob_start, alice_start) = tokio::join!(
            bob.start(),
            alice.start()
        );
        assert!(bob_start.is_ok());
        assert!(alice_start.is_ok());

        tokio::join!(
            wait_until_ready(&bob),
            wait_until_ready(&alice)
        );

        bob.clear_contacts().await.unwrap();
        alice.clear_contacts().await.unwrap();

        let session_keypair = KeyPair::random();

        let rc = bob.add_friend(
            alice_id,
            session_keypair.clone(),
            Some("Hello,Alice".to_string())
        ).await;
        assert!(rc.is_ok());

        let rc = alice.add_friend(
            bob_id,
            session_keypair,
            Some("Hello,Bob".to_string())
        ).await;
        assert!(rc.is_ok());

        let bob_contacts = bob.get_contacts().await.unwrap();
        let bob_friends: Vec<_> = bob_contacts.iter()
            .filter(|contact| contact.contact_type() == ContactType::Friend)
            .collect();

        assert_eq!(bob_friends.len(), 1);
        assert_eq!(bob_friends[0].id(), &alice_id);
        assert_eq!(bob_friends[0].remark(), Some("Hello,Alice"));

        let rc = bob.get_contact(&alice_id).await;
        let contact = rc.unwrap();
        assert!(contact.is_some());

        let contact = contact.as_ref().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert_eq!(contact.remark(), Some("Hello,Alice"));

        let alice_contacts = alice.get_contacts().await.unwrap();
        let alice_friends: Vec<_> = alice_contacts.iter()
            .filter(|contact| contact.contact_type() == ContactType::Friend)
            .collect();
        assert_eq!(alice_friends.len(), 1);
        assert_eq!(alice_friends[0].id(), &bob_id);
        assert_eq!(alice_friends[0].remark(), Some("Hello,Bob"));


        let rc = alice.get_contact(&bob_id).await;
        assert!(rc.is_ok());
        let contact = rc.unwrap();
        assert!(contact.is_some());

        let contact = contact.as_ref().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert_eq!(contact.remark(), Some("Hello,Bob"));

        bob.clear_contacts().await.unwrap();
        alice.clear_contacts().await.unwrap();

        let (bob_stop, alice_stop) = tokio::join!(
            bob.stop(),
            alice.stop()
        );
        assert!(bob_stop.is_ok());
        assert!(alice_stop.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_message() {
        let options = Options::load(BOB_YAML).unwrap();
        let sender = Client::new(options);
        let sender_id = sender.user_id().clone();

        let (received_tx, mut received_rx) = mpsc::unbounded_channel();
        let options = build_options_with_message_listener(
            Options::load(ALICE_YAML).unwrap(),
            MessageCapture { received: received_tx },
        );
        let recipient = Client::new(options);
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

    #[tokio::test]
    #[serial]
    async fn test_channel_message() {
        let (received_tx, mut received_rx) = mpsc::unbounded_channel();
        let options = build_options_with_message_listener(
             Options::load(BOB_YAML).unwrap(),
            ChannelMessageCapture { received: received_tx }
        );
        let bob = Client::new(options);
        let bob_id = bob.user_id().clone();

        let options = Options::load(ALICE_YAML).unwrap();
        let alice = Client::new(options);
        let alice_id = alice.user_id().clone();

        assert_ne!(bob_id, alice_id);
        let (bob_start, alice_start) = tokio::join!(
            bob.start(),
            alice.start()
        );
        assert!(bob_start.is_ok());
        assert!(alice_start.is_ok());

        tokio::join!(
            wait_until_ready(&bob),
            wait_until_ready(&alice)
        );

        let mut created_channel_id = None;
        let mut alice_joined = false;
        let routine = async {
        let rc = bob.get_contacts().await;
        assert!(rc.is_ok());
        for contact in rc.unwrap() {
            if contact.contact_type() == ContactType::Channel && contact.name() == Some("API channel test") {
                let rc = bob.remove_channel(contact.id()).await;
                assert!(rc.is_ok(), "Removing leftover testcase channel failed: {:?}", rc.err());
            }
        }

        bob.clear_contacts().await.unwrap();
        alice.clear_contacts().await.unwrap();
        bob.clear_friend_requests().await.unwrap();
        alice.clear_friend_requests().await.unwrap();

        let rc = bob.friend_request(alice_id, Some("channel test")).await;
        assert!(rc.is_ok());
        let rc = bob.get_friend_request(alice_id).await;
        assert!(rc.is_ok());
        assert!(rc.as_ref().unwrap().is_some());

        let request = rc.unwrap().unwrap();
        wait_for_friend_request(
            &alice,
            bob_id,
            request.created_at(),
            false
        ).await;

        let rc = alice.accept_friend_request(bob_id).await;
        assert!(rc.is_ok());

        wait_for_friend_request(
            &bob,
            alice_id,
            request.created_at(),
            true
        ).await;

        let rc = bob.get_contact(&alice_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_some());

        let rc = alice.get_contact(&bob_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_some());

        let rc = bob.create_channel(
            Permission::OwnerInvite,
            "API channel test",
            None::<String>,
            false,
        ).await;
        assert!(rc.is_ok(), "Creating testcase channel failed: {:?}", rc.err());

        let channel = rc.unwrap();
        let channel_id = channel.id().clone();
        created_channel_id = Some(channel_id);

        assert!(channel.has_member(&bob_id));

        let rc = bob.create_invite_ticket(&channel_id, Some(alice_id)).await;
        assert!(rc.is_ok());
        let invite_ticket = rc.unwrap();

        let rc = alice.join_channel(invite_ticket).await;
        assert!(rc.is_ok());
        alice_joined = true;

        let joined = rc.unwrap();
        assert_eq!(joined.id(), &channel_id);
        assert!(joined.has_member(&alice_id));
        assert!(joined.has_member(&bob_id));

        let body = "Alice channel message to Bob";
        let rc = alice
            .message(Some(channel_id))
            .text_body(body)
            .send()
            .await;

        assert!(rc.is_ok());
        let sent = rc.unwrap();
        assert_eq!(sent.from(), Some(&alice_id));
        assert_eq!(sent.recipient(), &channel_id);

        let received = time::timeout(
            Duration::from_secs(15),
            received_rx.recv()
        ).await;
        assert!(received.is_ok());
        let received = received.unwrap();
        assert!(received.is_some());

        let (id, from, recipient, body_bytes) = received.unwrap();
        assert_eq!(&id, sent.id());
        assert_eq!(from, Some(alice_id));
        assert_eq!(recipient, channel_id);
        assert_eq!(body_bytes, sent.payload_as_content().map(|content| content.body().to_vec()));
        assert_eq!(sent.payload_as_content().unwrap().as_text(), Some(body));

        let rc = alice.leave_channel(&channel_id).await;
        assert!(rc.is_ok());
        alice_joined = false;
        let rc = alice.get_contact(&channel_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        let rc = bob.remove_channel(&channel_id).await;
        assert!(rc.is_ok());
        created_channel_id = None;
        let rc = bob.get_contact(&channel_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        bob.clear_contacts().await.unwrap();
        alice.clear_contacts().await.unwrap();
        bob.clear_friend_requests().await.unwrap();
        alice.clear_friend_requests().await.unwrap();

        };
        use futures::FutureExt;
        let result = std::panic::AssertUnwindSafe(routine).catch_unwind().await;
        if let Some(channel_id) = created_channel_id {
            if alice_joined {
                if let Err(error) = alice.leave_channel(&channel_id).await {
                    eprintln!("Leaving testcase channel during cleanup failed: {error}");
                }
            }
            if let Err(error) = bob.remove_channel(&channel_id).await {
                eprintln!("Removing testcase channel during cleanup failed: {error}");
            }
        }

        let (bob_stop, alice_stop) = tokio::join!(
            bob.stop(),
            alice.stop()
        );
        if let Err(panic) = result {
            if let Err(error) = &bob_stop {
                eprintln!("Stopping Bob during cleanup failed: {error}");
            }
            if let Err(error) = &alice_stop {
                eprintln!("Stopping Alice during cleanup failed: {error}");
            }
            std::panic::resume_unwind(panic);
        }
        assert!(bob_stop.is_ok());
        assert!(alice_stop.is_ok());
    }
}
