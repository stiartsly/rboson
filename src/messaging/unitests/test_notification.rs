use serde_cbor::Value;
use crate::messaging::internal::notification::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::channel::{Permission, Role};
    use serde_cbor::Value;

    fn channel() -> ChannelInfo {
        ChannelInfo::new(
            Id::random(), Id::random(), Id::random(), None, Permission::Public,
            Some("channel".into()), None, false, 10, 11, Vec::new(),
        )
    }

    #[test]
    fn notifications_roundtrip_all_event_payloads_directly() {
        let id = Id::random();
        let session = serde_cbor::value::from_value(Value::Map([
            (Value::Text("id".into()), Value::Bytes(id.as_bytes().to_vec())),
        ].into_iter().collect())).unwrap();
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
            assert_eq!(NotificationEvent::try_from(event.value()).unwrap(), event);
            let notification = Notification { id, source: Id::random(), timestamp: 42, body };
            let bytes = serde_cbor::to_vec(&notification).unwrap();
            let Value::Map(wire) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
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
    fn notifications_accept_absent_or_null_body_only_for_empty_events() {
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

    #[test]
    fn notifications_reject_invalid_timestamps_events_and_payloads() {
        for (timestamp, event, body) in [
            (-1, "cd", None),
            (u64::MAX.into(), "cd", None),
            (42, "unknown", None),
            (42, "sn", None),
            (42, "cs", Some(Value::Map(Default::default()))),
            (42, "cc", Some(Value::Integer(1))),
            (42, "cd", Some(Value::Integer(1))),
            (42, "cot", Some(Value::Bytes(vec![0; 1]))),
            (42, "csr", Some(Value::Integer(1))),
            (42, "ciu", Some(Value::Array(Vec::new()))),
            (42, "cru", Some(Value::Map(Default::default()))),
            (42, "cmb", Some(Value::Array(vec![Value::Integer(1)]))),
            (42, "cmj", Some(Value::Map(Default::default()))),
            (42, "cml", Some(Value::Bytes(vec![0; 1]))),
        ] {
            let mut wire = [
                (Value::Text("id".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
                (Value::Text("s".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
                (Value::Text("t".into()), Value::Integer(timestamp)),
                (Value::Text("e".into()), Value::Text(event.into())),
            ].into_iter().collect::<std::collections::BTreeMap<_, _>>();
            if let Some(body) = body {
                wire.insert(Value::Text("b".into()), body);
            }
            assert!(serde_cbor::from_slice::<Notification>(&serde_cbor::to_vec(&Value::Map(wire)).unwrap()).is_err(), "{event}");
        }
    }
}
