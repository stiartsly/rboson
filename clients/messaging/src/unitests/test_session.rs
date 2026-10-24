// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#[cfg(test)]
mod session_tests {
    use super::*;

    fn agent() -> Rc<SessionAgent> {
        Session::new(crate::verticle::tests::options()).unwrap()
    }

    #[tokio::test]
    async fn tracks_incoming_requests() {
        let agent = agent();
        let friend = *CryptoIdentity::new().id();
        {
            agent.on_friend_request(friend, *agent.user_id(), "hello".into(), 1234, false);
            agent.on_friend_request_accepted(friend, 1235, true);
        }
        let request = agent.get_friend_request(&friend).await.unwrap().unwrap();
        assert_eq!(request.user_id(), &friend);
        assert_eq!(request.initiator_id(), agent.user_id());
        assert_eq!(request.hello(), Some("hello"));
        assert!(request.is_accepted());
        assert!(!request.is_expired());
        assert_eq!(request.accepted_at(), Some(UNIX_EPOCH + Duration::from_millis(1235)));
        assert_eq!(agent.get_contact(&friend).await.unwrap().unwrap().contact_type(), ContactType::Friend);
        agent.remove_friend_requests(&[friend]).await.unwrap();
        assert!(agent.get_friend_request(&friend).await.unwrap().is_none());
        assert!(request.is_accepted());
        agent.remove_contact(&friend).await.unwrap();
        assert!(agent.get_contacts().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sends_outgoing_requests() {
        let agent = agent();
        let (mqtt, _eventloop) = AsyncClient::new(MqttOptions::new("test", "localhost", 1883), 8);
        *agent.mqtt.lock().unwrap() = Some(mqtt);
        let friend = *CryptoIdentity::new().id();
        agent.friend_request(&friend, None).await.unwrap();
        let request = agent.get_friend_request(&friend).await.unwrap().unwrap();
        assert_eq!(request.hello(), None);
        assert!(agent.accept_friend_request(&friend).await.is_err());
        agent.remove_friend_request(&friend).await.unwrap();
        {
            agent.on_friend_request(friend, friend, "hello".into(), 1234, false);
        }
        agent.accept_friend_request(&friend).await.unwrap();
        assert!(agent.get_friend_request(&friend).await.unwrap().unwrap().is_accepted());
        assert!(agent.friend_sessions.lock().unwrap().contains_key(&friend));
        assert!(agent.accept_friend_request(&friend).await.is_err());

        let message = agent.message(Some(friend))
            .content_type("image/png")
            .binary_body(vec![0, 1, 2])
            .send().await.unwrap();
        assert_eq!(message.from(), Some(agent.user_id()));
        assert_eq!(message.recipient(), &friend);
        assert_eq!(message.payload_as_content().unwrap().content_type(), "image/png");
        let wire: MessageContent = serde_cbor::from_slice(message.payload_as_bytes()).unwrap();
        assert_eq!(wire.body, Value::Bytes(vec![0, 1, 2]));
        agent.clear_contacts().await.unwrap();
        assert!(agent.friend_sessions.lock().unwrap().is_empty());
        assert!(agent.message(Some(friend)).text_body("hello").send().await.is_err());
        agent.clear_friend_requests().await.unwrap();
        assert!(agent.get_friend_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn preserves_failed_requests() {
        let agent = agent();
        let friend = *CryptoIdentity::new().id();
        assert!(agent.friend_request(&friend, None).await.is_err());
        assert!(agent.friend_request(agent.user_id(), None).await.is_err());
        assert!(agent.add_friend(&friend, vec![0], None).await.is_err());
        assert!(agent.accept_friend_request(&friend).await.is_err());
        assert!(agent.get_friend_requests().await.unwrap().is_empty());
        assert!(agent.get_contacts().await.unwrap().is_empty());
        assert!(agent.mqtt.lock().unwrap().is_none());
        assert!(agent.friend_sessions.lock().unwrap().is_empty());
        assert!(!agent.is_running());
        assert!(!agent.is_connected());
        assert!(!agent.is_ready());
    }
}
