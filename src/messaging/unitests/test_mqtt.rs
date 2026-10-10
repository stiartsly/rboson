#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Mutex, thread::{self, ThreadId}};
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};

    #[derive(Clone, Default)]
    struct ThreadRecorder(Arc<Mutex<Vec<(&'static str, ThreadId)>>>);

    impl ThreadRecorder {
        fn record(&self, event: &'static str) {
            self.0.lock().unwrap().push((event, thread::current().id()));
        }
    }

    impl ConnectionListener for ThreadRecorder {
        fn on_connecting(&self) {
            self.record("connecting");
        }
        fn on_connected(&self) {
            self.record("connected");
        }
        fn on_ready(&self) {
            self.record("ready");
        }
        fn on_disconnected(&self) {
            self.record("disconnected");
        }
    }

    impl crate::messaging::FriendRequestListener for ThreadRecorder {
        fn on_friend_request(&self, _user_id: &Id, _hello: Option<&str>) {
            self.record("friend_request");
        }
    }

    impl ContactListener for ThreadRecorder {
        fn on_contact_added(&self, _contact: &dyn Contact) {
            self.record("contact_added");
        }
    }

    fn options(endpoint: &str, peer_id: Id, recorder: ThreadRecorder) -> VerticleOptions {
        let mut builder = Options::builder();
        builder.with_peer_id(peer_id);
        builder.with_peer_endpoint(endpoint).unwrap();
        builder.with_user_keypair(crate::signature::KeyPair::random());
        builder.with_device_keypair(crate::signature::KeyPair::random());
        builder.with_data_dir(std::env::temp_dir());
        builder.with_connection_listener(recorder.clone());
        builder.with_friend_request_listener(recorder.clone());
        builder.with_contact_listener(recorder);
        let options = Arc::new(builder.build().unwrap());
        VerticleOptions {
            connection_listener: options.connection_listener(),
            message_listener: options.message_listener(),
            channel_listener: options.channel_listener(),
            contact_listener: options.contact_listener(),
            session_listener: options.session_listener(),
            friend_request_listener: options.friend_request_listener(),
            options,
        }
    }

    async fn read_packet(stream: &mut TcpStream) -> (u8, Vec<u8>) {
        let header = stream.read_u8().await.unwrap();
        let mut length = 0usize;
        let mut shift = 0;
        loop {
            let byte = stream.read_u8().await.unwrap();
            length |= usize::from(byte & 127) << shift;
            if byte & 128 == 0 {
                break;
            }
            shift += 7;
            assert!(shift < 28);
        }
        let mut body = vec![0; length];
        stream.read_exact(&mut body).await.unwrap();
        (header, body)
    }

    async fn write_packet(stream: &mut TcpStream, header: u8, body: &[u8]) {
        let mut packet = vec![header];
        let mut length = body.len();
        loop {
            let mut byte = (length % 128) as u8;
            length /= 128;
            if length > 0 {
                byte |= 128;
            }
            packet.push(byte);
            if length == 0 {
                break;
            }
        }
        packet.extend_from_slice(body);
        stream.write_all(&packet).await.unwrap();
    }

    #[tokio::test]
    async fn merged_session_handles_mqtt_and_requests_on_one_thread() {
        task::LocalSet::new().run_until(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                let thread_id = thread::current().id();
                let recorder = ThreadRecorder::default();
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let peer = CryptoIdentity::new();
                let friend = CryptoIdentity::new();
                let target = CryptoIdentity::new();
                let friend_id = *friend.id();
                let target_id = *target.id();
                let session = MqttSession::new(options(
                    &format!("mqtt://{}", listener.local_addr().unwrap()),
                    *peer.id(),
                    recorder.clone(),
                )).unwrap();
                let user_id = *session.user_id();
                let device_id = *session.device_id();
                let (published_tx, published_rx) = tokio::sync::oneshot::channel();
                let broker = task::spawn_local(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    assert_eq!(read_packet(&mut stream).await.0 >> 4, 1);
                    write_packet(&mut stream, 0x20, &[0, 0]).await;
                    let (header, body) = read_packet(&mut stream).await;
                    assert_eq!(header >> 4, 8);
                    write_packet(&mut stream, 0x90, &[body[0], body[1], 1, 1, 1]).await;

                    let handshake = Handshake {
                        timestamp: 1234,
                        handshake_type: HandshakeType::FriendRequest,
                        body: Value::Text("incoming".into()),
                    };
                    let wire = WireMessage {
                        version: MESSAGE_VERSION,
                        id: MqttSession::message_id(&friend_id, 1234).unwrap(),
                        recipient: user_id,
                        message_type: HANDSHAKE_MESSAGE,
                        from: Some(friend_id),
                        created_at: 1234,
                        payload: Value::Bytes(friend.encrypt_into(
                            &user_id,
                            &serde_cbor::to_vec(&handshake).unwrap(),
                        ).unwrap()),
                    };
                    let payload = peer.encrypt_into(
                        &device_id,
                        &serde_cbor::to_vec(&wire).unwrap(),
                    ).unwrap();
                    let mut body = (USER_INBOX.len() as u16).to_be_bytes().to_vec();
                    body.extend_from_slice(USER_INBOX.as_bytes());
                    body.extend_from_slice(&payload);
                    write_packet(&mut stream, 0x30, &body).await;

                    let mut session_identity = None;
                    for index in 0..3 {
                        let (header, body) = read_packet(&mut stream).await;
                        assert_eq!(header, 0x32);
                        let topic_length = usize::from(u16::from_be_bytes([body[0], body[1]]));
                        assert_eq!(&body[2..2 + topic_length], DEVICE_OUTBOX.as_bytes());
                        let offset = 2 + topic_length;
                        let wire: WireMessage = serde_cbor::from_slice(
                            &peer.decrypt_into(&device_id, &body[offset + 2..]).unwrap(),
                        ).unwrap();
                        let Value::Bytes(payload) = wire.payload else {
                            panic!("Outgoing payload should be binary");
                        };
                        if index == 0 {
                            assert_eq!(wire.recipient, target_id);
                            let handshake: Handshake = serde_cbor::from_slice(
                                &target.decrypt_into(&user_id, &payload).unwrap(),
                            ).unwrap();
                            assert!(matches!(handshake.handshake_type, HandshakeType::FriendRequest));
                            assert_eq!(handshake.body, Value::Text("outgoing".into()));
                        } else if index == 1 {
                            assert_eq!(wire.recipient, friend_id);
                            let handshake: Handshake = serde_cbor::from_slice(
                                &friend.decrypt_into(&user_id, &payload).unwrap(),
                            ).unwrap();
                            assert!(matches!(handshake.handshake_type, HandshakeType::FriendRequestAccept));
                            let Value::Bytes(key) = handshake.body else {
                                panic!("Accepted friend request should carry a session key");
                            };
                            session_identity = Some(CryptoIdentity::try_from(key.as_slice()).unwrap());
                        } else {
                            assert_eq!(wire.recipient, friend_id);
                            assert_eq!(wire.message_type, crate::messaging::message::MessageType::ContentMessage as u8);
                            let content: MessageContent = serde_cbor::from_slice(
                                &session_identity.as_ref().unwrap().decrypt_into(&user_id, &payload).unwrap(),
                            ).unwrap();
                            assert!(matches!(content.format, ContentFormat::Binary));
                            assert_eq!(content.body, Value::Bytes(vec![0, 1, 2]));
                            assert_eq!(content.headers.get("Content-Type"), Some(&Value::Text("image/png".into())));
                        }
                        write_packet(&mut stream, 0x40, &body[offset..offset + 2]).await;
                    }
                    published_tx.send(()).unwrap();
                    let mut buffer = [0; 2];
                    if let Err(error) = stream.read(&mut buffer).await {
                        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
                    }
                });

                let (first, second) = tokio::join!(session.start(), session.start());
                first.unwrap();
                second.unwrap();
                while !session.is_ready() || session.get_friend_request(&friend_id).await.unwrap().is_none() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                session.friend_request(&target_id, Some("outgoing".into())).await.unwrap();
                let request = session.get_friend_request(&target_id).await.unwrap().unwrap();
                assert_eq!(request.initiator_id(), &user_id);
                assert_eq!(request.hello(), Some("outgoing"));
                session.accept_friend_request(&friend_id).await.unwrap();
                assert!(session.get_friend_request(&friend_id).await.unwrap().unwrap().is_accepted());
                assert!(session.accept_friend_request(&friend_id).await.is_err());
                assert_eq!(session.get_contact(&friend_id).await.unwrap().unwrap().contact_type(), ContactType::Friend);
                let message = session.message(Some(friend_id))
                    .content_type("image/png")
                    .binary_body(vec![0, 1, 2])
                    .send().await.unwrap();
                assert_eq!(message.recipient(), &friend_id);
                published_rx.await.unwrap();
                session.stop().await.unwrap();
                session.stop().await.unwrap();
                broker.await.unwrap();
                assert!(!session.is_running());
                assert!(!session.is_connected());
                assert!(!session.is_ready());
                assert!(session.mqtt.lock().unwrap().is_none());
                assert!(session.mqtt_task.borrow().is_none());
                let events = recorder.0.lock().unwrap();
                for event in ["connecting", "connected", "ready", "friend_request", "contact_added", "disconnected"] {
                    assert_eq!(events.iter().filter(|(name, _)| *name == event).count(), 1);
                }
                assert!(events.iter().all(|(_, id)| *id == thread_id));
            }).await.unwrap();
        }).await;
    }

    #[tokio::test]
    async fn merged_session_preserves_state_and_reports_request_failures() {
        let session = MqttSession::new(options(
            "mqtt://127.0.0.1:1883",
            *CryptoIdentity::new().id(),
            ThreadRecorder::default(),
        )).unwrap();
        let friend = *CryptoIdentity::new().id();
        assert!(session.friend_request(&friend, None).await.is_err());
        assert!(session.friend_request(session.user_id(), None).await.is_err());
        assert!(session.accept_friend_request(&friend).await.is_err());
        assert!(session.add_friend(&friend, vec![0], None).await.is_err());
        assert!(session.get_friend_requests().await.unwrap().is_empty());
        assert!(session.get_contacts().await.unwrap().is_empty());
        session.on_friend_request(friend, friend, "hello".into(), 1234, true);
        assert_eq!(session.get_friend_request(&friend).await.unwrap().unwrap().hello(), Some("hello"));
        session.remove_friend_requests(&[friend]).await.unwrap();
        assert!(session.get_friend_request(&friend).await.unwrap().is_none());
        let message = session.message(Some(friend)).text_body("hello");
        let (mqtt, _eventloop) = AsyncClient::new(MqttOptions::new("test", "localhost", 1883), 8);
        *session.mqtt.lock().unwrap() = Some(mqtt);
        session.add_friend(
            &friend,
            crate::signature::KeyPair::random().private_key().as_ref().to_vec(),
            Some("friend".into()),
        ).await.unwrap();
        assert!(session.friend_sessions.lock().unwrap().contains_key(&friend));
        assert!(message.send().await.is_ok());
        let message = session.message(Some(friend)).text_body("hello");
        session.friend_remove(friend).await.unwrap();
        assert!(!session.friend_sessions.lock().unwrap().contains_key(&friend));
        assert!(session.get_contacts().await.unwrap().is_empty());
        assert!(message.send().await.is_err());
    }

    fn handshake_payload(
        session: &MqttSession,
        peer: &CryptoIdentity,
        friend: &CryptoIdentity,
        topic: &str,
        handshake: Handshake,
    ) -> Vec<u8> {
        let payload = serde_cbor::to_vec(&handshake).unwrap();
        let (from, recipient, payload) = if topic == USER_OUTBOX {
            (*session.user_id(), *friend.id(), session.user_identity.encrypt_into(friend.id(), &payload).unwrap())
        } else {
            (*friend.id(), *session.user_id(), friend.encrypt_into(session.user_id(), &payload).unwrap())
        };
        let wire = WireMessage {
            version: MESSAGE_VERSION,
            id: MqttSession::message_id(peer.id(), handshake.timestamp).unwrap(),
            recipient,
            message_type: HANDSHAKE_MESSAGE,
            from: Some(from),
            created_at: handshake.timestamp,
            payload: Value::Bytes(payload),
        };
        peer.encrypt_into(session.device_id(), &serde_cbor::to_vec(&wire).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn duplicate_and_stale_friend_requests_preserve_state() {
        let recorder = ThreadRecorder::default();
        let session = MqttSession::new(options(
            "mqtt://127.0.0.1:1883",
            *CryptoIdentity::new().id(),
            recorder.clone(),
        )).unwrap();
        let friend = *CryptoIdentity::new().id();
        session.on_friend_request(friend, friend, "hello".into(), 1234, true);
        session.on_friend_request(friend, friend, "duplicate".into(), 1234, true);
        session.on_friend_request(friend, friend, "stale".into(), 1233, true);
        assert_eq!(session.get_friend_request(&friend).await.unwrap().unwrap().hello(), Some("hello"));
        session.on_friend_request_accepted(friend, 1235, false);
        session.on_friend_request_accepted(friend, 1236, false);
        session.remove_contact(&friend).await.unwrap();
        session.on_friend_request(friend, friend, "replayed".into(), 1234, true);
        let request = session.get_friend_request(&friend).await.unwrap().unwrap();
        assert!(request.is_accepted());
        assert_eq!(request.accepted_at(), Some(UNIX_EPOCH + Duration::from_millis(1235)));
        let events = recorder.0.lock().unwrap();
        assert_eq!(events.iter().filter(|(name, _)| *name == "friend_request").count(), 1);
        assert_eq!(events.iter().filter(|(name, _)| *name == "contact_added").count(), 1);
    }

    #[tokio::test]
    async fn outbox_handshakes_sync_requests_and_acceptance_without_rotating_keys() {
        let peer = CryptoIdentity::new();
        let friend = CryptoIdentity::new();
        let session = MqttSession::new(options(
            "mqtt://127.0.0.1:1883", *peer.id(), ThreadRecorder::default(),
        )).unwrap();
        let request = handshake_payload(&session, &peer, &friend, USER_OUTBOX, Handshake {
            timestamp: 1234,
            handshake_type: HandshakeType::FriendRequest,
            body: Value::Text("outgoing".into()),
        });
        session.handle_user_publish(peer.id(), USER_OUTBOX, &request).unwrap();
        let request = session.get_friend_request(friend.id()).await.unwrap().unwrap();
        assert_eq!(request.initiator_id(), session.user_id());
        assert_eq!(request.hello(), Some("outgoing"));

        let key = crate::signature::KeyPair::random();
        let acceptance = handshake_payload(&session, &peer, &friend, USER_INBOX, Handshake {
            timestamp: 1235,
            handshake_type: HandshakeType::FriendRequestAccept,
            body: Value::Bytes(key.private_key().as_ref().to_vec()),
        });
        session.handle_user_publish(peer.id(), USER_INBOX, &acceptance).unwrap();
        let duplicate = handshake_payload(&session, &peer, &friend, USER_INBOX, Handshake {
            timestamp: 1236,
            handshake_type: HandshakeType::FriendRequestAccept,
            body: Value::Bytes(crate::signature::KeyPair::random().private_key().as_ref().to_vec()),
        });
        session.handle_user_publish(peer.id(), USER_INBOX, &duplicate).unwrap();
        assert_eq!(session.friend_sessions.lock().unwrap().get(friend.id()).unwrap().id(), &Id::from(key.public_key()));
        assert_eq!(session.get_friend_request(friend.id()).await.unwrap().unwrap().accepted_at(), Some(UNIX_EPOCH + Duration::from_millis(1235)));
        assert_eq!(session.get_contacts().await.unwrap().len(), 1);

        let receiver = MqttSession::new(options(
            "mqtt://127.0.0.1:1883", *peer.id(), ThreadRecorder::default(),
        )).unwrap();
        receiver.on_friend_request(*friend.id(), *friend.id(), "incoming".into(), 1234, false);
        let acceptance = handshake_payload(&receiver, &peer, &friend, USER_OUTBOX, Handshake {
            timestamp: 1235,
            handshake_type: HandshakeType::FriendRequestAccept,
            body: Value::Bytes(key.private_key().as_ref().to_vec()),
        });
        receiver.handle_user_publish(peer.id(), USER_OUTBOX, &acceptance).unwrap();
        assert!(receiver.get_friend_request(friend.id()).await.unwrap().unwrap().is_accepted());
        assert_eq!(receiver.get_contacts().await.unwrap().len(), 1);
        assert_eq!(receiver.friend_sessions.lock().unwrap().get(friend.id()).unwrap().id(), &Id::from(key.public_key()));
    }

    #[tokio::test]
    async fn unexpected_and_stale_acceptance_leave_state_unchanged() {
        let peer = CryptoIdentity::new();
        let friend = CryptoIdentity::new();
        let session = MqttSession::new(options(
            "mqtt://127.0.0.1:1883", *peer.id(), ThreadRecorder::default(),
        )).unwrap();
        let acceptance = handshake_payload(&session, &peer, &friend, USER_INBOX, Handshake {
            timestamp: 1234,
            handshake_type: HandshakeType::FriendRequestAccept,
            body: Value::Bytes(crate::signature::KeyPair::random().private_key().as_ref().to_vec()),
        });
        assert!(session.handle_user_publish(peer.id(), USER_INBOX, &acceptance).is_err());
        session.on_friend_request(*friend.id(), *friend.id(), "incoming".into(), 1233, false);
        assert!(session.handle_user_publish(peer.id(), USER_INBOX, &acceptance).is_err());
        session.remove_friend_request(friend.id()).await.unwrap();
        session.on_friend_request(*friend.id(), *session.user_id(), "outgoing".into(), 1235, false);
        assert!(session.handle_user_publish(peer.id(), USER_INBOX, &acceptance).is_err());
        assert!(!session.get_friend_request(friend.id()).await.unwrap().unwrap().is_accepted());
        assert!(session.friend_sessions.lock().unwrap().is_empty());
        assert!(session.get_contacts().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn publish_failure_does_not_accept_or_create_friend_requests() {
        let session = MqttSession::new(options(
            "mqtt://127.0.0.1:1883", *CryptoIdentity::new().id(), ThreadRecorder::default(),
        )).unwrap();
        let (mqtt, eventloop) = AsyncClient::new(MqttOptions::new("closed", "localhost", 1883), 8);
        drop(eventloop);
        *session.mqtt.lock().unwrap() = Some(mqtt);
        let friend = *CryptoIdentity::new().id();
        assert!(session.friend_request(&friend, None).await.is_err());
        assert!(session.get_friend_requests().await.unwrap().is_empty());
        session.on_friend_request(friend, friend, "incoming".into(), 1234, false);
        assert!(session.accept_friend_request(&friend).await.is_err());
        assert!(!session.get_friend_request(&friend).await.unwrap().unwrap().is_accepted());
        assert!(session.friend_sessions.lock().unwrap().is_empty());
        assert!(session.get_contacts().await.unwrap().is_empty());
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Clone, Default)]
    struct Observed {
        events: Arc<Mutex<Vec<(&'static str, Id)>>>,
        messages: Arc<Mutex<Vec<(bool, Vec<u8>)>>>,
    }

    impl MessageListener for Observed {
        fn on_message(&self, message: &dyn Message) {
            self.messages.lock().unwrap().push((false, message.payload_as_bytes().to_vec()));
        }
        fn on_sent(&self, message: &dyn Message) {
            self.messages.lock().unwrap().push((true, message.payload_as_bytes().to_vec()));
        }
    }

    impl ContactListener for Observed {
        fn on_contact_added(&self, contact: &dyn Contact) {
            self.events.lock().unwrap().push(("add", *contact.id()));
        }
        fn on_contacts_updated(&self, contacts: &[Box<dyn Contact>]) {
            self.events.lock().unwrap().extend(contacts.iter().map(|contact| ("update", *contact.id())));
        }
        fn on_contacts_removed(&self, ids: &[Id]) {
            self.events.lock().unwrap().extend(ids.iter().map(|id| ("remove", *id)));
        }
    }

    impl SessionListener for Observed {
        fn on_new_session(&self, session: &SessionInfo) {
            self.events.lock().unwrap().push(("session", *session.device_id()));
        }
    }

    fn session(observed: &Observed) -> Rc<MqttSession> {
        let mut options = crate::messaging::verticle::tests::options();
        options.message_listener = Arc::new(observed.clone());
        options.contact_listener = Arc::new(observed.clone());
        options.session_listener = Arc::new(observed.clone());
        MqttSession::new(options).unwrap()
    }

    fn opaque(session: &MqttSession, id: Id, revision: i32, key: &crate::signature::KeyPair, blocked: bool) -> OpaqueContact {
        let encrypted_key = session.user_identity.encrypt_into(session.user_id(), key.private_key().as_ref()).unwrap();
        let data = ContactData {
            id,
            kind: 1,
            session_key: Some(Value::Bytes(encrypted_key)),
            name: Some("friend".into()),
            remark: None,
            tags: None,
            muted: false,
            blocked,
            created_at: 100,
            updated_at: 100 + i64::from(revision),
        };
        let data = serde_cbor::to_vec(&data).unwrap();
        OpaqueContact {
            id,
            revision,
            data: Value::Bytes(session.user_identity.encrypt_into(session.user_id(), &data).unwrap()),
        }
    }

    #[test]
    fn origin_timestamps_are_unique_and_packet_limit_includes_mqtt_headers() {
        let clock = Mutex::new(0);
        let first = MqttSession::origin_timestamp(&clock).unwrap();
        let second = MqttSession::origin_timestamp(&clock).unwrap();
        assert!(second > first);
        assert!(MqttSession::validate_outbox_packet(MAX_MESSAGE_SIZE - 11).is_ok());
        assert!(MqttSession::validate_outbox_packet(MAX_MESSAGE_SIZE - 10).is_err());
        assert!(MqttSession::validate_outbox_packet(usize::MAX).is_err());
    }

    #[tokio::test]
    async fn home_peer_notifications_are_routed_and_untrusted_control_is_rejected() {
        let observed = Observed::default();
        let session = session(&observed);
        let peer = CryptoIdentity::new();
        let other = Id::random();
        let info: SessionInfo = serde_cbor::value::from_value(Value::Map([
            (Value::Text("id".into()), serde_cbor::value::to_value(other).unwrap()),
            (Value::Text("o".into()), Value::Bool(true)),
        ].into_iter().collect())).unwrap();
        let notification = Notification {
            id: Id::random(), source: other, timestamp: 1234, event: "sn".into(),
            body: Some(serde_cbor::value::to_value(info).unwrap()),
        };
        let mut wire = WireMessage {
            version: MESSAGE_VERSION, id: Id::random(), recipient: *session.user_id(),
            from: Some(*peer.id()), created_at: 1234,
            message_type: crate::messaging::message::MessageType::StateMessage as u8,
            payload: Value::Bytes(serde_cbor::to_vec(&notification).unwrap()),
        };
        let envelope = peer.encrypt_into(session.device_id(), &serde_cbor::to_vec(&wire).unwrap()).unwrap();
        session.handle_user_publish(peer.id(), USER_INBOX, &envelope).unwrap();
        assert_eq!(*observed.events.lock().unwrap(), vec![("session", other)]);
        let sync = Notification {
            id: Id::random(), source: other, timestamp: 1234, event: "cs".into(),
            body: Some(serde_cbor::value::to_value(ContactSync {
                revision: 1, kind: 2, mutations: vec![], contacts: vec![],
            }).unwrap()),
        };
        wire.payload = Value::Bytes(serde_cbor::to_vec(&sync).unwrap());
        let envelope = peer.encrypt_into(session.device_id(), &serde_cbor::to_vec(&wire).unwrap()).unwrap();
        session.handle_user_publish(peer.id(), USER_INBOX, &envelope).unwrap();
        assert_eq!(session.contacts_revision.get(), 1);
        wire.from = Some(*CryptoIdentity::new().id());
        wire.message_type = crate::messaging::message::MessageType::ControlMessage as u8;
        let envelope = peer.encrypt_into(session.device_id(), &serde_cbor::to_vec(&wire).unwrap()).unwrap();
        assert!(session.handle_user_publish(peer.id(), DEVICE_INBOX, &envelope).is_err());
        assert_eq!(*observed.events.lock().unwrap(), vec![("session", other)]);
    }

    #[tokio::test]
    async fn rejected_subscription_does_not_mark_the_session_ready() {
        task::LocalSet::new().run_until(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                let session = session(&Observed::default());
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let (mqtt, eventloop) = AsyncClient::new(MqttOptions::new(
                    "rejected", "127.0.0.1", listener.local_addr().unwrap().port(),
                ), 8);
                *session.mqtt.lock().unwrap() = Some(mqtt.clone());
                session.running.set(true);
                let agent = session.clone();
                let runner = task::spawn_local(agent.run_mqtt(mqtt, eventloop));
                let broker = task::spawn_local(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    assert_eq!(read_packet(&mut stream).await.0 >> 4, 1);
                    stream.write_all(&[0x20, 2, 0, 0]).await.unwrap();
                    let (header, body) = read_packet(&mut stream).await;
                    assert_eq!(header >> 4, 8);
                    stream.write_all(&[0x90, 5, body[0], body[1], 1, 128, 1]).await.unwrap();
                    let mut buffer = [0; 1];
                    if let Err(error) = stream.read(&mut buffer).await {
                        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
                    }
                });
                runner.await.unwrap();
                broker.await.unwrap();
                assert!(!session.is_ready());
                assert!(!session.is_connected());
                assert!(!session.is_running());
                assert!(session.mqtt.lock().unwrap().is_none());
                session.stop().await.unwrap();
            }).await.unwrap();
        }).await;
    }

    #[tokio::test]
    async fn encrypted_contact_sync_is_atomic_and_revision_checked() {
        let observed = Observed::default();
        let session = session(&observed);
        let friend = *CryptoIdentity::new().id();
        let key = crate::signature::KeyPair::random();
        session.apply_contact_sync(ContactSync {
            revision: 1, kind: 2, mutations: vec![],
            contacts: vec![opaque(&session, friend, 1, &key, false)],
        }).unwrap();
        assert_eq!(session.contacts_revision.get(), 1);
        assert_eq!(session.friend_sessions.lock().unwrap().get(&friend).unwrap().id(), &Id::from(key.public_key()));
        let other = *CryptoIdentity::new().id();
        session.apply_contact_sync(ContactSync {
            revision: 2, kind: 1, contacts: vec![],
            mutations: vec![ContactMutation {
                revision: 1, op: "a".into(),
                data: Some(serde_cbor::value::to_value(opaque(&session, other, 2, &key, false)).unwrap()),
            }],
        }).unwrap();
        let replay = ContactSync {
            revision: 2, kind: 1, contacts: vec![],
            mutations: vec![ContactMutation { revision: 1, op: "c".into(), data: None }],
        };
        session.apply_contact_sync(replay).unwrap();
        assert_eq!(session.get_contacts().await.unwrap().len(), 2);
        assert_eq!(observed.events.lock().unwrap().len(), 2);
        let invalid = OpaqueContact { id: friend, revision: 3, data: Value::Bytes(vec![0]) };
        assert!(session.apply_contact_sync(ContactSync {
            revision: 3, kind: 2, mutations: vec![],
            contacts: vec![opaque(&session, other, 3, &key, false), invalid],
        }).is_err());
        assert_eq!(session.contacts_revision.get(), 2);
        assert_eq!(session.get_contacts().await.unwrap().len(), 2);
        assert_eq!(observed.events.lock().unwrap().len(), 2);
        assert!(session.apply_contact_sync(ContactSync {
            revision: 4, kind: 1, contacts: vec![],
            mutations: vec![ContactMutation { revision: 3, op: "c".into(), data: None }],
        }).is_err());
        assert!(session.contact_sync_needed.get());
        assert_eq!(session.contacts_revision.get(), 2);
        session.apply_contact_sync(ContactSync {
            revision: 3, kind: 1, contacts: vec![],
            mutations: vec![ContactMutation {
                revision: 2, op: "r".into(),
                data: Some(serde_cbor::value::to_value(vec![friend]).unwrap()),
            }],
        }).unwrap();
        assert!(!session.friend_sessions.lock().unwrap().contains_key(&friend));
        assert!(observed.events.lock().unwrap().contains(&("remove", friend)));
        assert!(session.password().unwrap().ends_with("?contactsRevision=3"));
    }

    #[tokio::test]
    async fn synced_blocking_applies_to_live_builders_and_friend_acceptance() {
        let session = session(&Observed::default());
        let friend = *CryptoIdentity::new().id();
        let key = crate::signature::KeyPair::random();
        let builder = session.message(Some(friend)).text_body("hello");
        session.apply_contact_sync(ContactSync {
            revision: 1, kind: 2, mutations: vec![],
            contacts: vec![opaque(&session, friend, 1, &key, true)],
        }).unwrap();
        assert!(builder.send().await.err().expect("Blocked send must fail").to_string().contains("blocked"));
        session.on_friend_request(friend, friend, "hello".into(), 100, true);
        assert!(session.get_friend_requests().await.unwrap().is_empty());
        assert!(session.accept_friend_request(&friend).await.is_err());
    }

    #[tokio::test]
    async fn inbox_and_other_device_outbox_content_use_the_correct_keys() {
        let observed = Observed::default();
        let session = session(&observed);
        let peer = CryptoIdentity::new();
        let friend = CryptoIdentity::new();
        let key = crate::signature::KeyPair::random();
        session.register_friend_session(*friend.id(), key.private_key().as_ref()).unwrap();
        let content = MessageContent { headers: HashMap::new(), format: ContentFormat::Binary, body: Value::Bytes(vec![0, 255]) };
        let plaintext = serde_cbor::to_vec(&content).unwrap();
        for outbox in [false, true] {
            let from = if outbox { *session.user_id() } else { *friend.id() };
            let payload = if outbox {
                session.user_identity.encrypt_into(&Id::from(key.public_key()), &plaintext).unwrap()
            } else {
                friend.encrypt_into(&Id::from(key.public_key()), &plaintext).unwrap()
            };
            let message = WireMessage {
                version: MESSAGE_VERSION, id: Id::random(),
                recipient: if outbox { *friend.id() } else { *session.user_id() },
                from: Some(from), created_at: 1234,
                message_type: crate::messaging::message::MessageType::ContentMessage as u8,
                payload: Value::Bytes(payload),
            };
            let envelope = peer.encrypt_into(session.device_id(), &serde_cbor::to_vec(&message).unwrap()).unwrap();
            session.handle_user_publish(peer.id(), if outbox { USER_OUTBOX } else { USER_INBOX }, &envelope).unwrap();
        }
        let messages = observed.messages.lock().unwrap();
        assert_eq!(messages.len(), 2);
        assert!(!messages[0].0 && messages[1].0);
        for (_, payload) in messages.iter() {
            assert_eq!(serde_cbor::from_slice::<MessageContent>(payload).unwrap().body, Value::Bytes(vec![0, 255]));
        }
    }

    #[tokio::test]
    async fn rpc_errors_cancellation_and_shutdown_clean_pending_calls() {
        let session = session(&Observed::default());
        let (mqtt, _eventloop) = AsyncClient::new(MqttOptions::new("rpc", "localhost", 1883), 8);
        *session.mqtt.lock().unwrap() = Some(mqtt);
        session.connected.set(true);
        let call = session.get_sessions();
        tokio::pin!(call);
        tokio::select! {
            _ = &mut call => panic!("RPC completed without a response"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
        let id = *session.pending_rpc.borrow().keys().next().unwrap();
        let response = RpcResponse {
            id, method: "sl".into(), result: None,
            error: Some(RpcError { code: 401, message: "denied".into() }),
        };
        session.handle_rpc_response(Value::Bytes(serde_cbor::to_vec(&response).unwrap())).unwrap();
        assert!(call.await.unwrap_err().to_string().contains("401"));
        assert!(session.pending_rpc.borrow().is_empty());
        assert!(tokio::time::timeout(Duration::from_millis(10), session.get_sessions()).await.is_err());
        assert!(session.pending_rpc.borrow().is_empty());
        let call = session.get_sessions();
        tokio::pin!(call);
        tokio::select! {
            _ = &mut call => panic!("RPC completed without a response"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
        session.stop().await.unwrap();
        assert!(call.await.unwrap_err().to_string().contains("stopped"));
        assert!(session.pending_rpc.borrow().is_empty());
    }

    #[tokio::test]
    async fn rpc_timeout_is_bounded_and_removes_the_registration() {
        let session = session(&Observed::default());
        let (mqtt, _eventloop) = AsyncClient::new(MqttOptions::new("timeout", "localhost", 1883), 8);
        *session.mqtt.lock().unwrap() = Some(mqtt);
        session.connected.set(true);
        let started = tokio::time::Instant::now();
        let result = tokio::time::timeout(RPC_TIMEOUT + Duration::from_secs(2), session.get_sessions()).await.unwrap();
        assert!(result.unwrap_err().to_string().contains("timed out"));
        assert!(started.elapsed() >= RPC_TIMEOUT);
        assert!(session.pending_rpc.borrow().is_empty());
    }

    async fn read_packet(stream: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
        let header = stream.read_u8().await.unwrap();
        let mut length = 0usize;
        let mut shift = 0;
        loop {
            let byte = stream.read_u8().await.unwrap();
            length |= usize::from(byte & 127) << shift;
            if byte & 128 == 0 {
                break;
            }
            shift += 7;
            assert!(shift < 28);
        }
        let mut body = vec![0; length];
        stream.read_exact(&mut body).await.unwrap();
        (header, body)
    }

    async fn publish(stream: &mut tokio::net::TcpStream, topic: &str, payload: &[u8]) {
        let mut body = (topic.len() as u16).to_be_bytes().to_vec();
        body.extend_from_slice(topic.as_bytes());
        body.extend_from_slice(payload);
        let mut packet = vec![0x30];
        let mut length = body.len();
        loop {
            let mut byte = (length % 128) as u8;
            length /= 128;
            if length > 0 {
                byte |= 128;
            }
            packet.push(byte);
            if length == 0 {
                break;
            }
        }
        packet.extend_from_slice(&body);
        stream.write_all(&packet).await.unwrap();
    }

    #[tokio::test]
    async fn session_rpc_and_contact_sync_round_trip_through_device_inbox() {
        task::LocalSet::new().run_until(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                let peer = CryptoIdentity::new();
                let observed = Observed::default();
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let mut builder = Options::builder();
                builder.with_peer_id(*peer.id());
                builder.with_peer_endpoint(format!("mqtt://{}", listener.local_addr().unwrap())).unwrap();
                builder.with_user_keypair(crate::signature::KeyPair::random());
                builder.with_device_keypair(crate::signature::KeyPair::random());
                builder.with_data_dir(std::env::temp_dir());
                let mut options = crate::messaging::verticle::tests::options();
                options.options = Arc::new(builder.build().unwrap());
                options.contact_listener = Arc::new(observed.clone());
                let session = MqttSession::new(options).unwrap();
                let device = *session.device_id();
                let user = *session.user_id();
                let other_device = Id::random();
                let friend = *CryptoIdentity::new().id();
                let key = crate::signature::KeyPair::random();
                let sync = ContactSync {
                    revision: 2, kind: 2, mutations: vec![],
                    contacts: vec![opaque(&session, friend, 2, &key, false)],
                };
                let expected_session: SessionInfo = serde_cbor::value::from_value(Value::Map([
                    (Value::Text("id".into()), serde_cbor::value::to_value(other_device).unwrap()),
                    (Value::Text("o".into()), Value::Bool(true)),
                    (Value::Text("lt".into()), Value::Integer(1234)),
                    (Value::Text("la".into()), Value::Text("127.0.0.1:1883".into())),
                ].into_iter().collect())).unwrap();
                let expected = expected_session.clone();
                let broker = task::spawn_local(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    assert_eq!(read_packet(&mut stream).await.0 >> 4, 1);
                    stream.write_all(&[0x20, 2, 0, 0]).await.unwrap();
                    let (header, body) = read_packet(&mut stream).await;
                    assert_eq!(header >> 4, 8);
                    stream.write_all(&[0x90, 5, body[0], body[1], 1, 1, 1]).await.unwrap();
                    let mut last_id = 0;
                    for method in ["sl", "sr", "cs"] {
                        let (header, body) = read_packet(&mut stream).await;
                        assert_eq!(header, 0x32);
                        let topic_length = usize::from(u16::from_be_bytes([body[0], body[1]]));
                        assert_eq!(&body[2..2 + topic_length], DEVICE_OUTBOX.as_bytes());
                        let offset = 2 + topic_length;
                        let packet_id = &body[offset..offset + 2];
                        stream.write_all(&[0x40, 2, packet_id[0], packet_id[1]]).await.unwrap();
                        let envelope = peer.decrypt_into(&device, &body[offset + 2..]).unwrap();
                        let message: WireMessage = serde_cbor::from_slice(&envelope).unwrap();
                        assert_eq!(message.recipient, *peer.id());
                        assert_eq!(message.message_type, crate::messaging::message::MessageType::ControlMessage as u8);
                        assert!(message.from.is_none());
                        let Value::Bytes(payload) = message.payload else { panic!("RPC payload should be binary") };
                        let request: RpcRequest = serde_cbor::from_slice(&payload).unwrap();
                        assert_eq!(request.method, method);
                        assert!(request.id > last_id);
                        last_id = request.id;
                        let result = match method {
                            "sl" => {
                                assert!(request.params.is_none());
                                Some(serde_cbor::value::to_value(vec![expected.clone()]).unwrap())
                            }
                            "sr" => {
                                assert_eq!(request.params, Some(serde_cbor::value::to_value(other_device).unwrap()));
                                None
                            }
                            "cs" => {
                                assert_eq!(request.params, Some(Value::Integer(0)));
                                Some(serde_cbor::value::to_value(&sync).unwrap())
                            }
                            _ => unreachable!(),
                        };
                        let response = RpcResponse { id: request.id, method: method.into(), result, error: None };
                        let wire = WireMessage {
                            version: MESSAGE_VERSION, id: Id::random(), recipient: user,
                            from: Some(*peer.id()), created_at: request.id,
                            message_type: crate::messaging::message::MessageType::ControlMessage as u8,
                            payload: Value::Bytes(serde_cbor::to_vec(&response).unwrap()),
                        };
                        let payload = peer.encrypt_into(&device, &serde_cbor::to_vec(&wire).unwrap()).unwrap();
                        publish(&mut stream, DEVICE_INBOX, &payload).await;
                        if method == "sr" {
                            let notification = Notification {
                                id: Id::random(), source: other_device, timestamp: request.id, event: "cs".into(),
                                body: Some(serde_cbor::value::to_value(ContactSync {
                                    revision: 2, kind: 1, contacts: vec![],
                                    mutations: vec![ContactMutation { revision: 1, op: "c".into(), data: None }],
                                }).unwrap()),
                            };
                            let wire = WireMessage {
                                version: MESSAGE_VERSION, id: Id::random(), recipient: user,
                                from: Some(*peer.id()), created_at: request.id,
                                message_type: crate::messaging::message::MessageType::StateMessage as u8,
                                payload: Value::Bytes(serde_cbor::to_vec(&notification).unwrap()),
                            };
                            let payload = peer.encrypt_into(&device, &serde_cbor::to_vec(&wire).unwrap()).unwrap();
                            publish(&mut stream, USER_INBOX, &payload).await;
                        }
                    }
                    let mut buffer = [0; 1];
                    if let Err(error) = stream.read(&mut buffer).await {
                        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
                    }
                });
                session.start().await.unwrap();
                while !session.is_ready() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                assert_eq!(session.get_sessions().await.unwrap(), vec![expected_session]);
                assert!(session.revoke_session(session.device_id()).await.is_err());
                session.revoke_session(&other_device).await.unwrap();
                while session.contacts_revision.get() != 2 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                assert_eq!(session.contacts_revision.get(), 2);
                assert_eq!(session.friend_sessions.lock().unwrap().get(&friend).unwrap().id(), &Id::from(key.public_key()));
                assert!(observed.events.lock().unwrap().contains(&("add", friend)));
                session.stop().await.unwrap();
                broker.await.unwrap();
                assert!(session.pending_rpc.borrow().is_empty());
            }).await.unwrap();
        }).await;
    }
}