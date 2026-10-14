#[cfg(test)]
mod tests {
    use std::time::Duration;
    use serial_test::serial;
    use tokio::time;
    use tokio::sync::mpsc;

    use boson::{Id, signature::KeyPair};
    use boson::messaging::{Channel, ChannelListener, Client, Message, MessageListener, Options, Permission};
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

    fn create_client(config: &str) -> Client {
        Client::new(Options::load(config).unwrap())
    }

    fn create_client_with_message_listener(
        config: &str,
        listener: impl MessageListener + 'static,
    ) -> Client {
        let options = build_options_with_message_listener(
            Options::load(config).unwrap(),
            listener,
        );
        Client::new(options)
    }

    fn create_client_with_channel_listener(
        config: &str,
        listener: impl ChannelListener + 'static,
    ) -> Client {
        let options = Options::load(config).unwrap();
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
        builder.with_channel_listener(listener);
        Client::new(builder.build().unwrap())
    }

    async fn start_clients(first: &Client, second: &Client) {
        assert_ne!(first.user_id(), second.user_id());
        let (first_start, second_start) = tokio::join!(
            first.start(),
            second.start()
        );
        assert!(first_start.is_ok());
        assert!(second_start.is_ok());

        tokio::join!(
            wait_until_ready(first),
            wait_until_ready(second)
        );
    }

    async fn check_friend_requests(client: &Client, user_id: Id, accepted: bool) {
        let requests = client.get_friend_requests().await.unwrap();
        let request = requests.iter()
            .find(|request| request.user_id() == &user_id)
            .expect("The friend request should be present in the request list");
        assert_eq!(request.is_accepted(), accepted);
    }

    async fn check_friend_contact(client: &Client, user_id: &Id) {
        let rc = client.get_contact(user_id).await;
        assert!(rc.is_ok());

        let contact = rc.unwrap();
        assert!(contact.is_some());
        assert_eq!(contact.unwrap().contact_type(), ContactType::Friend);
    }

    async fn make_friends_by_request(bob: &Client, alice: &Client) {
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        let rc = bob.friend_request(alice_id, Some("hello")).await;
        assert!(rc.is_ok());

        let rc = bob.get_friend_request(alice_id).await;
        assert!(rc.is_ok());
        assert!(rc.as_ref().unwrap().is_some());

        let outgoing_request = rc.unwrap().unwrap();
        assert_eq!(outgoing_request.initiator_id(), &bob_id);
        assert_eq!(outgoing_request.hello(), Some("hello"));
        assert!(!outgoing_request.is_accepted());
        check_friend_requests(bob, alice_id, false).await;

        let incoming = wait_for_friend_request(
            alice,
            bob_id,
            outgoing_request.created_at(),
            false,
        ).await;
        assert_eq!(incoming.hello(), Some("hello"));
        check_friend_requests(alice, bob_id, false).await;

        let rc = alice.accept_friend_request(bob_id).await;
        assert!(rc.is_ok());

        wait_for_friend_request(
            bob,
            alice_id,
            outgoing_request.created_at(),
            true,
        ).await;
        wait_for_friend_request(
            alice,
            bob_id,
            outgoing_request.created_at(),
            true,
        ).await;

        check_friend_requests(bob, alice_id, true).await;
        check_friend_requests(alice, bob_id, true).await;
        check_friend_contact(bob, &alice_id).await;
        check_friend_contact(alice, &bob_id).await;
    }

    async fn make_friends_with_keypair(
        bob: &Client,
        alice: &Client,
        session_keypair: KeyPair,
    ) {
        bob.add_friend(
            *alice.user_id(),
            session_keypair.clone(),
            Some("hello,alice"),
        ).await.unwrap();

        alice.add_friend(
            *bob.user_id(),
            session_keypair,
            Some("hello,bob"),
        ).await.unwrap();

        check_friend_contact(bob, alice.user_id()).await;
        check_friend_contact(alice, bob.user_id()).await;
    }

    async fn create_channel_with_member(
        bob: &Client,
        alice: &Client,
        name: &str,
    ) -> (Id, Box<dyn Channel>) {
        for contact in bob.get_contacts().await.unwrap() {
            if contact.contact_type() == ContactType::Channel && contact.name() == Some(name) {
                bob.remove_channel(contact.id()).await.unwrap();
            }
        }
        clear_friend_requests_and_contacts(bob, alice).await;
        let channel = bob.create_channel(
            Permission::OwnerInvite,
            name,
            None::<String>,
            false,
        ).await.unwrap();
        let channel_id = *channel.id();
        assert!(channel.has_member(bob.user_id()));

        let ticket = bob.create_invite_ticket(&channel_id, Some(*alice.user_id()))
            .await.unwrap();
        let joined = alice.join_channel(ticket).await.unwrap();
        assert_eq!(joined.id(), &channel_id);
        assert!(joined.has_member(alice.user_id()));
        (channel_id, channel)
    }

    async fn clear_friend_requests_and_contacts(first: &Client, second: &Client) {
        let (first_result, second_result) = tokio::join!(
            async {
                let contacts = first.clear_contacts().await;
                let requests = first.clear_friend_requests().await;
                (contacts, requests)
            },
            async {
                let contacts = second.clear_contacts().await;
                let requests = second.clear_friend_requests().await;
                (contacts, requests)
            },
        );
        let (first_contacts, first_requests) = first_result;
        let (second_contacts, second_requests) = second_result;
        assert!(first_contacts.is_ok());
        assert!(second_contacts.is_ok());
        assert!(first_requests.is_ok());
        assert!(second_requests.is_ok());
    }

    async fn stop_two_clients(first: &Client, second: &Client) -> (bool, bool) {
        let (first_stop, second_stop) = tokio::join!(
            first.stop(),
            second.stop(),
        );
        (first_stop.is_ok(), second_stop.is_ok())
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

    struct ChannelRotationCapture {
        rotated: tokio::sync::mpsc::UnboundedSender<Id>,
    }

    impl ChannelListener for ChannelRotationCapture {
        fn on_channel_session_key_rotated(&self, channel: &dyn Channel) {
            self.rotated.send(*channel.id())
                .expect("Channel rotation testcase receiver should remain open");
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
        let bob = create_client(BOB_YAML);
        let alice = create_client(ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        clear_friend_requests_and_contacts(&bob, &alice).await;

        let session_keypair = KeyPair::random();
        make_friends_with_keypair(
            &bob,
            &alice,
            session_keypair
        ).await;

        let bob_contacts = bob.get_contacts().await.unwrap();
        let bob_friends: Vec<_> = bob_contacts.iter()
            .filter(|contact| contact.contact_type() == ContactType::Friend)
            .collect();

        assert_eq!(bob_friends.len(), 1);
        assert_eq!(bob_friends[0].id(), &alice_id);
        assert_eq!(bob_friends[0].remark(), Some("hello,alice"));

        let rc = bob.get_contact(&alice_id).await;
        let contact = rc.unwrap();
        assert!(contact.is_some());

        let contact = contact.as_ref().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert_eq!(contact.remark(), Some("hello,alice"));

        let alice_contacts = alice.get_contacts().await.unwrap();
        let alice_friends: Vec<_> = alice_contacts.iter()
            .filter(|contact| contact.contact_type() == ContactType::Friend)
            .collect();
        assert_eq!(alice_friends.len(), 1);
        assert_eq!(alice_friends[0].id(), &bob_id);
        assert_eq!(alice_friends[0].remark(), Some("hello,bob"));


        let rc = alice.get_contact(&bob_id).await;
        assert!(rc.is_ok());
        let contact = rc.unwrap();
        assert!(contact.is_some());

        let contact = contact.as_ref().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert_eq!(contact.remark(), Some("hello,bob"));

        clear_friend_requests_and_contacts(&bob, &alice).await;
        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        assert!(bob_stop);
        assert!(alice_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_conversation() {
        let bob = create_client(BOB_YAML);
        let bob_id = *bob.user_id();

        let (received_tx, mut received_rx) = mpsc::unbounded_channel();
        let alice = create_client_with_message_listener(
            ALICE_YAML,
            MessageCapture { received: received_tx },
        );
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        clear_friend_requests_and_contacts(&bob, &alice).await;
        let session_keypair = KeyPair::random();

        make_friends_with_keypair(
            &bob,
            &alice,
            session_keypair,
        ).await;

        let rc = bob.get_conversation(&alice_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        let rc = alice.get_conversation(&bob_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        let rc = bob.get_conversations().await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_empty());

        let rc = alice.get_conversations().await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_empty());

        let body = "conversation API verification";

        let rc = bob.message(Some(alice_id)).text_body(body).send().await;
        assert!(rc.is_ok());
        let received = time::timeout(Duration::from_secs(15), received_rx.recv())
            .await
            .expect("The conversation test message should arrive")
            .expect("The message listener should capture the incoming message");
        assert_eq!(received, Some(body.as_bytes().to_vec()));

        let rc = bob.get_conversation(&alice_id).await;
        assert!(rc.is_ok());
        let conversation_opt = rc.unwrap();
        assert!(conversation_opt.is_some());

        let bob_conversation = conversation_opt.unwrap();
        assert_eq!(bob_conversation.id(), &alice_id);
        assert_eq!(bob_conversation.contact().id(), &alice_id);
        assert_eq!(bob_conversation.preview(), body);
        assert_eq!(bob_conversation.message_count(), 1);
        assert_eq!(bob_conversation.unread_count(), 0);

        let rc = bob.get_conversations().await;
        assert!(rc.is_ok());
        let bob_conversations = rc.unwrap();
        assert_eq!(bob_conversations.len(), 1);
        assert_eq!(bob_conversations[0].id(), &alice_id);

        let rc = alice.get_conversation(&bob_id).await;
        assert!(rc.is_ok());
        let converstation_opt = rc.unwrap();
        assert!(converstation_opt.is_some());

        let alice_conversation = converstation_opt.unwrap();
        assert_eq!(alice_conversation.id(), &bob_id);
        assert_eq!(alice_conversation.contact().id(), &bob_id);
        assert_eq!(alice_conversation.preview(), body);
        assert_eq!(alice_conversation.message_count(), 1);
        assert_eq!(alice_conversation.unread_count(), 1);

        let rc = alice.get_conversations().await;
        assert!(rc.is_ok());
        let alice_conversations = rc.unwrap();
        assert_eq!(alice_conversations.len(), 1);
        assert_eq!(alice_conversations[0].id(), &bob_id);

        let rc = bob.remove_conversation(&alice_id).await;
        assert!(rc.is_ok());
        let rc = bob.get_conversation(&alice_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());

        let rc = bob.get_conversations().await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_empty());

        let rc = alice.remove_conversations(&[bob_id]).await;
        assert!(rc.is_ok());
        let rc = alice.get_conversation(&bob_id).await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_none());
        let rc = alice.get_conversations().await;
        assert!(rc.is_ok());
        assert!(rc.unwrap().is_empty());

        clear_friend_requests_and_contacts(&bob, &alice).await;
        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        assert!(bob_stop);
        assert!(alice_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_message() {
        let sender = create_client(BOB_YAML);
        let sender_id = sender.user_id().clone();

        let (received_tx, mut received_rx) = mpsc::unbounded_channel();
        let recipient = create_client_with_message_listener(
            ALICE_YAML,
            MessageCapture { received: received_tx },
        );
        let recipient_id = *recipient.user_id();

        start_clients(&sender, &recipient).await;
        clear_friend_requests_and_contacts(&sender, &recipient).await;
        make_friends_by_request(&sender, &recipient).await;

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

        clear_friend_requests_and_contacts(&sender, &recipient).await;
        let (sender_stop, recipient_stop) = stop_two_clients(&sender, &recipient).await;
        assert!(sender_stop);
        assert!(recipient_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_rotate_channel_session_key() {
        let (rotated_tx, mut rotated_rx) = mpsc::unbounded_channel();
        let bob = create_client_with_channel_listener(
            BOB_YAML,
            ChannelRotationCapture { rotated: rotated_tx },
        );
        let bob_id = *bob.user_id();
        assert!(bob.start().await.is_ok());
        wait_until_ready(&bob).await;

        let mut created_channel_id = None;
        let routine = async {
            for contact in bob.get_contacts().await.unwrap() {
                if contact.contact_type() == ContactType::Channel
                    && contact.name() == Some("API channel rotation test")
                {
                    bob.remove_channel(contact.id()).await.unwrap();
                }
            }
            bob.clear_contacts().await.unwrap();
            bob.clear_friend_requests().await.unwrap();

            let channel = bob.create_channel(
                Permission::OwnerInvite,
                "API channel rotation test",
                None::<String>,
                false,
            ).await.unwrap();
            let channel_id = *channel.id();
            created_channel_id = Some(channel_id);
            assert!(channel.has_member(&bob_id));

            let previous_session_id = *bob.create_invite_ticket(&channel_id, None)
                .await.unwrap().session_id();
            assert!(bob.rotate_channel_session_key(&channel_id).await.is_ok());
            let rotated_channel = time::timeout(Duration::from_secs(15), rotated_rx.recv())
                .await
                .expect("The channel owner should receive the rotation callback")
                .expect("The rotation listener should remain open");
            assert_eq!(rotated_channel, channel_id);

            let rotated_session_id = *bob.create_invite_ticket(&channel_id, None)
                .await.unwrap().session_id();
            assert_ne!(previous_session_id, rotated_session_id);
        };

        use futures::FutureExt;
        let result = std::panic::AssertUnwindSafe(routine).catch_unwind().await;
        if let Some(channel_id) = created_channel_id {
            if let Err(error) = bob.remove_channel(&channel_id).await {
                eprintln!("Removing rotation testcase channel during cleanup failed: {error}");
            }
        }
        let contacts = bob.clear_contacts().await;
        let requests = bob.clear_friend_requests().await;
        let bob_stop = bob.stop().await.is_ok();
        if let Err(panic) = result {
            if let Err(error) = &contacts {
                eprintln!("Clearing Bob's contacts during rotation testcase cleanup failed: {error}");
            }
            if let Err(error) = &requests {
                eprintln!("Clearing Bob's friend requests during rotation testcase cleanup failed: {error}");
            }
            if !bob_stop {
                eprintln!("Stopping Bob during rotation testcase cleanup failed");
            }
            std::panic::resume_unwind(panic);
        }
        assert!(contacts.is_ok());
        assert!(requests.is_ok());
        assert!(bob_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_update_channel_info() {
        let bob = create_client(BOB_YAML);
        let alice = create_client(ALICE_YAML);
        start_clients(&bob, &alice).await;
        let (channel_id, channel) =
            create_channel_with_member(&bob, &alice, "API channel info test").await;
        let updated = channel.edit_channel()
            .name("API channel info test update".to_string())
            .notice(Some("Updated channel notice".to_string()))
            .announced(true)
            .permission(Permission::Public)
            .build();
        let rc = bob.update_channel_info(updated.as_ref()).await;
        assert!(rc.is_ok());

        let contact = bob.get_contact(&channel_id).await.unwrap().unwrap();
        assert_eq!(contact.name(), Some("API channel info test update"));

        assert!(bob.remove_channel(&channel_id).await.is_ok());
        clear_friend_requests_and_contacts(&bob, &alice).await;
        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        assert!(bob_stop);
        assert!(alice_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_set_channel_member_role() {
        let bob = create_client(BOB_YAML);
        let alice = create_client(ALICE_YAML);
        start_clients(&bob, &alice).await;
        let (channel_id, _) =
            create_channel_with_member(&bob, &alice, "API channel role test").await;
        let alice_id = *alice.user_id();

        assert!(bob.set_channel_members_role(&channel_id, &[alice_id], "Moderator").await.is_ok());
        assert!(bob.set_channel_members_role(&channel_id, &[alice_id], "Member").await.is_ok());
        assert!(bob.set_channel_members_role(&channel_id, &[alice_id], "Owner").await.is_err());

        assert!(bob.remove_channel(&channel_id).await.is_ok());
        clear_friend_requests_and_contacts(&bob, &alice).await;
        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        assert!(bob_stop);
        assert!(alice_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_ban_and_unban_channel_member() {
        let bob = create_client(BOB_YAML);
        let alice = create_client(ALICE_YAML);
        start_clients(&bob, &alice).await;
        let (channel_id, _) =
            create_channel_with_member(&bob, &alice, "API channel ban test").await;
        let alice_id = *alice.user_id();

        assert!(bob.ban_channel_members(&channel_id, &[alice_id]).await.is_ok());
        assert!(bob.unban_channel_members(&channel_id, &[alice_id]).await.is_ok());

        assert!(bob.remove_channel(&channel_id).await.is_ok());
        clear_friend_requests_and_contacts(&bob, &alice).await;
        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        assert!(bob_stop);
        assert!(alice_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_remove_channel_member() {
        let bob = create_client(BOB_YAML);
        let alice = create_client(ALICE_YAML);
        start_clients(&bob, &alice).await;
        let (channel_id, _) =
            create_channel_with_member(&bob, &alice, "API channel member removal test").await;
        let alice_id = *alice.user_id();

        assert!(bob.remove_channel_members(&channel_id, &[alice_id]).await.is_ok());
        assert!(alice.leave_channel(&channel_id).await.is_err());

        assert!(bob.remove_channel(&channel_id).await.is_ok());
        clear_friend_requests_and_contacts(&bob, &alice).await;
        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        assert!(bob_stop);
        assert!(alice_stop);
    }

    #[tokio::test]
    #[serial]
    async fn test_channel_message() {
        let (received_tx, mut received_rx) = mpsc::unbounded_channel();
        let bob = create_client_with_message_listener(
            BOB_YAML,
            ChannelMessageCapture { received: received_tx },
        );
        let bob_id = bob.user_id().clone();

        let alice = create_client(ALICE_YAML);
        let alice_id = alice.user_id().clone();

        start_clients(&bob, &alice).await;

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

        clear_friend_requests_and_contacts(&bob, &alice).await;
        make_friends_by_request(&bob, &alice).await;

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

        clear_friend_requests_and_contacts(&bob, &alice).await;

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

        let (bob_stop, alice_stop) = stop_two_clients(&bob, &alice).await;
        if let Err(panic) = result {
            if !bob_stop {
                eprintln!("Stopping Bob during cleanup failed");
            }
            if !alice_stop {
                eprintln!("Stopping Alice during cleanup failed");
            }
            std::panic::resume_unwind(panic);
        }
        assert!(bob_stop);
        assert!(alice_stop);
    }
}
