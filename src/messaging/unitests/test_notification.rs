#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn notification_roundtrip_for_all_events() {
        for value in [1, 2, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33] {
            let event = NotificationEvent::try_from(value).unwrap();
            assert_eq!(event.value(), value);
            let body = match event {
                NotificationEvent::ChannelDelete | NotificationEvent::ChannelLeave => None,
                NotificationEvent::ChannelOwnershipTransfer | NotificationEvent::ChannelMemberLeave =>
                    Some(Value::Bytes(Id::random().as_bytes().to_vec())),
                NotificationEvent::ChannelSessionKeyRotate => Some(Value::Bytes(vec![1; 64])),
                NotificationEvent::ChannelMembersBan | NotificationEvent::ChannelMembersUnban
                    | NotificationEvent::ChannelMembersRemove => Some(Value::Array(Vec::new())),
                _ => Some(Value::Map(Default::default())),
            };
            let notification = Notification { id: Id::random(), source: Id::random(), timestamp: 42, event, body };
            let bytes = notification.serialize().unwrap();
            let wire: Value = serde_cbor::from_slice(&bytes).unwrap();
            let Value::Map(wire) = wire else { panic!("Notification must be a map") };
            assert_eq!(wire.get(&Value::Text("e".into())), Some(&Value::Text(event.as_str().into())));
            let parsed = Notification::parse(&bytes).unwrap();
            assert_eq!(parsed.id, notification.id);
            assert_eq!(parsed.source, notification.source);
            assert_eq!(parsed.timestamp, notification.timestamp);
            assert_eq!(parsed.event, event);
            assert_eq!(parsed.body, notification.body);
        }
        assert!(NotificationEvent::try_from(20).is_err());
        assert!(Notification::parse(&[0xff]).is_err());
        let bad = Notification {
            id: Id::random(), source: Id::random(), timestamp: 1,
            event: NotificationEvent::ChannelMemberLeave, body: Some(Value::Integer(1)),
        };
        assert!(bad.serialize().is_err());
        assert!(Notification::parse(&serde_cbor::to_vec(&bad).unwrap()).is_err());
    }
}
