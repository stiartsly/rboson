
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{CryptoIdentity, Identity};
    use crate::messaging::options::OptionsBuilder;
    use std::{collections::HashMap, time::Duration};

    pub(in crate::messaging) fn options() -> VerticleOptions {
        let mut builder = OptionsBuilder::new();
        builder.with_peer_id(*CryptoIdentity::new().id());
        builder.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
        builder.with_user_keypair(crate::signature::KeyPair::random());
        builder.with_device_keypair(crate::signature::KeyPair::random());
        builder.with_data_dir(std::env::temp_dir());
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

    fn client(options: VerticleOptions) -> VerticleClient {
        deploy(options).unwrap()
    }

    #[tokio::test]
    async fn events_execute_in_order_and_return_session_errors() {
        tokio::time::timeout(Duration::from_secs(5), async {
            let client = client(options());
            let friend = *CryptoIdentity::new().id();
            let (add_tx, add_rx) = oneshot::channel();
            let (get_tx, get_rx) = oneshot::channel();
            client.sender().send(VerticleEvent::RegisterFriendSession {
                user_id: friend,
                session_key: crate::signature::KeyPair::random().private_key().as_ref().to_vec(),
                remark: Some("friend".into()),
                complete: add_tx,
            }).unwrap();
            client.sender().send(VerticleEvent::GetContact {
                id: friend,
                complete: get_tx,
            }).unwrap();
            add_rx.await.unwrap().unwrap();
            let mut contact = get_rx.await.unwrap().unwrap().unwrap();
            assert_eq!(contact.remark(), Some("friend"));
            contact.remark = Some("updated".into());
            client.update_contact(contact).await.unwrap();
            let updated = client.get_contact(friend).await.unwrap().unwrap();
            assert_eq!(updated.remark(), Some("updated"));
            assert_eq!(updated.revision(), 2);
            assert_eq!(client.get_contacts().await.unwrap().len(), 1);

            let error = client.friend_request(friend, None).await.unwrap_err();
            assert!(error.to_string().contains("not running"));
            assert!(client.get_friend_request(friend).await.unwrap().is_none());
            assert!(client.get_friend_requests().await.unwrap().is_empty());
            assert!(client.friend_accept(friend).await.is_err());
            assert!(client.add_friend(friend, vec![0], None).await.is_err());
            assert!(client.friend_reject(friend).await.is_err());
            assert!(client.friend_info(friend).await.is_err());
            client.remove_friend_request(friend).await.unwrap();
            client.remove_friend_requests(vec![friend]).await.unwrap();
            client.clear_friend_requests().await.unwrap();
            client.friend_remove(friend).await.unwrap();
            assert!(client.get_contacts().await.unwrap().is_empty());
            client.remove_contacts(vec![friend]).await.unwrap();
            client.clear_contacts().await.unwrap();
            client.stop().await.unwrap();
            assert!(client.handle.lock().unwrap().is_none());
            assert!(client.get_contacts().await.is_err());
        }).await.unwrap();
    }

    #[tokio::test]
    async fn message_events_reject_invalid_input_without_stopping_verticle() {
        tokio::time::timeout(Duration::from_secs(5), async {
            let client = client(options());
            let friend = *CryptoIdentity::new().id();
            for (headers, body, text) in [
                (HashMap::from([("X-Test".into(), serde_json::Value::Bool(true))]), vec![0], false),
                (HashMap::new(), vec![255], true),
                (HashMap::new(), b"hello".to_vec(), true),
            ] {
                let (tx, rx) = oneshot::channel();
                client.sender().send(VerticleEvent::ContentMessage {
                    recipient: friend,
                    headers,
                    body,
                    text,
                    complete: tx,
                }).unwrap();
                assert!(rx.await.unwrap().is_err());
            }
            assert!(client.get_contacts().await.unwrap().is_empty());
            client.stop().await.unwrap();
        }).await.unwrap();
    }

    #[tokio::test]
    async fn startup_errors_are_returned_and_shutdown_finishes() {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut options = options();
            let mut builder = OptionsBuilder::new();
            builder.with_peer_id(*options.options.peer_id());
            builder.with_peer_endpoint(options.options.peer_endpoint().as_str()).unwrap();
            builder.with_user_keypair(options.options.user_key().clone());
            builder.with_device_keypair(options.options.device_key().clone());
            builder.with_data_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
            options.options = Arc::new(builder.build().unwrap());
            let client = client(options);
            assert!(client.start().await.is_err());
            let error = client.friend_request(*CryptoIdentity::new().id(), None).await.unwrap_err();
            assert!(error.to_string().contains("not running"));
            client.stop().await.unwrap();
            assert!(client.handle.lock().unwrap().is_none());
        }).await.unwrap();
    }

    #[tokio::test]
    async fn public_client_sends_friend_request_and_message_through_mqtt() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        async fn packet(stream: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
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

        tokio::time::timeout(Duration::from_secs(5), async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("mqtt://{}", listener.local_addr().unwrap());
            let broker = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                assert_eq!(packet(&mut stream).await.0 >> 4, 1);
                stream.write_all(&[0x20, 2, 0, 0]).await.unwrap();
                let (header, body) = packet(&mut stream).await;
                assert_eq!(header >> 4, 8);
                stream.write_all(&[0x90, 5, body[0], body[1], 1, 1, 1]).await.unwrap();
                for _ in 0..2 {
                    let (header, body) = packet(&mut stream).await;
                    assert_eq!(header >> 4, 3);
                    let topic_length = usize::from(u16::from_be_bytes([body[0], body[1]]));
                    assert_eq!(&body[2..2 + topic_length], b"d/o");
                    let offset = 2 + topic_length;
                    assert!(body.len() > offset + 2);
                    stream.write_all(&[0x40, 2, body[offset], body[offset + 1]]).await.unwrap();
                }
            });

            let options = options().options;
            let mut builder = OptionsBuilder::new();
            builder.with_peer_id(*options.peer_id());
            builder.with_peer_endpoint(endpoint).unwrap();
            builder.with_user_keypair(options.user_key().clone());
            builder.with_device_keypair(options.device_key().clone());
            builder.with_data_dir(std::env::temp_dir());
            let client = crate::messaging::client::Client::new(builder.build().unwrap());
            client.start().await.unwrap();
            while !client.is_ready() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }

            let friend = CryptoIdentity::new().id().clone();
            client.friend_request(friend, Some("hello".into())).await.unwrap();
            let request = client.get_friend_request(friend).await.unwrap().unwrap();
            assert_eq!(request.hello(), Some("hello"));
            assert_eq!(request.initiator_id(), client.user_id());
            client.add_friend(
                friend,
                crate::signature::KeyPair::random().private_key().as_ref().to_vec(),
                None,
            ).await.unwrap();
            let message = client.message(Some(friend))
                .content_type("image/png")
                .binary_body(vec![0, 1, 2])
                .send().await.unwrap();
            assert_eq!(message.payload_as_content().unwrap().content_type(), "image/png");
            broker.await.unwrap();
            client.stop().await.unwrap();
            assert!(!client.is_running());
        }).await.unwrap();
    }
}