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