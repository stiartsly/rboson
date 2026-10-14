#[cfg(test)]
mod tests {
    use serde_cbor::Value;
    use crate::messaging::channel::{Permission, Role};
    use crate::messaging::internal::{
        notification::*,
        notification::Body as NotificationBody,
        notification::Event as NotificationEvent,
    };


    fn channel() -> ChannelInfo {
        ChannelInfo::new(
            Id::random(),
            Id::random(),
            Id::random(),
            None,
            Permission::Public,
            Some("channel".into()),
            None,
            false,
            10,
            11,
            Vec::new(),
        )
    }

    #[test]
    fn test_notification_events() {
        let id = Id::random();
        let session = serde_cbor::value::from_value(
            Value::Map([
                (Value::Text("id".into()), Value::Bytes(id.as_bytes().to_vec())),
            ].into_iter().collect())
        ).unwrap();

        let bodies = [
            NotificationBody::SessionNew(session),
            NotificationBody::ContactSync(ContactSync::up_to_date(0).unwrap()),
            NotificationBody::ChannelCreate(channel()),
            NotificationBody::ChannelDelete,
            NotificationBody::ChannelJoin(channel()),
            NotificationBody::ChannelLeave,
            NotificationBody::ChannelOwnershipTransfer(id),
            NotificationBody::ChannelSessionKeyRotate(vec![7; 64]),
            NotificationBody::ChannelInfoUpdate(Default::default()),
            NotificationBody::ChannelMembersRoleUpdate(ChannelMembersRole::new(vec![id], Role::Moderator)),
            NotificationBody::ChannelMembersBan(vec![id]),
            NotificationBody::ChannelMembersUnban(vec![id]),
            NotificationBody::ChannelMembersRemove(vec![id]),
            NotificationBody::ChannelMemberJoin(ChannelMember::new(id, Role::Member, 12)),
            NotificationBody::ChannelMemberLeave(id),
        ];
        for body in bodies {
            let event = body.event();
            let rc = NotificationEvent::try_from(event);
            assert!(rc.is_ok());
            let decoded_event = rc.unwrap();
            assert_eq!(decoded_event, event);

            let notification = Notification {
                id,
                source: Id::random(),
                timestamp: 42,
                body
            };
            let rc = serde_cbor::to_vec(&notification);
            assert!(rc.is_ok());

            let bytes = rc.unwrap();
            let rc = serde_cbor::from_slice::<Value>(&bytes);
            assert!(rc.is_ok());
            let decoded = rc.unwrap();

            let Value::Map(wire) = decoded else {
                panic!("Notification must serialize as a map");
            };

            assert_eq!(wire.get(&Value::Text("e".into())), Some(&Value::Text(event.as_str().into())));
            assert_eq!(wire.get(&Value::Text("t".into())), Some(&Value::Integer(42)));
            if event == NotificationEvent::ChannelSessionKeyRotate {
                assert_eq!(wire.get(&Value::Text("b".into())), Some(&Value::Bytes(vec![7; 64])));
            }
            let decoded: Notification = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded.id, notification.id);
            assert_eq!(decoded.source, notification.source);
            assert_eq!(decoded.timestamp, notification.timestamp);
            assert_eq!(decoded.body.event(), event);
            assert_eq!(serde_cbor::value::to_value(&decoded).unwrap(), Value::Map(wire));
        }
    }

    #[test]
    fn test_notifications_accept_events() {
        for event in ["cd", "cl"] {
            for body in [None, Some(Value::Null)] {
                let mut wire = [
                    (Value::Text("id".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
                    (Value::Text("s".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
                    (Value::Text("t".into()), Value::Integer(42)),
                    (Value::Text("e".into()), Value::Text(event.into())),
                ].into_iter().collect::<std::collections::BTreeMap<_, _>>();
                if let Some(body) = body {
                    wire.insert(Value::Text("b".into()), body);
                }
                let decoded: Notification = serde_cbor::from_slice(&serde_cbor::to_vec(&Value::Map(wire)).unwrap()).unwrap();
                assert_eq!(decoded.body.event().as_str(), event);
            }
        }
    }
}
