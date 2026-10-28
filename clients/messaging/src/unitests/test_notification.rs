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

use serde_cbor::Value;
use crate::channel::{Permission, Role};
use crate::internal::{
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
fn notification_events() {
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
fn accepts_events() {
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
