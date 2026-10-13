use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use crate::{Id, errors::Result};
use crate::messaging::errors::EncodingError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub(crate) enum NotificationEvent {
    #[serde(rename = "sn")]
    SessionNew = 1,
    #[serde(rename = "cs")]
    ContactSync,
    #[serde(rename = "cc")]
    ChannelCreate = 21,
    #[serde(rename = "cd")]
    ChannelDelete,
    #[serde(rename = "cj")]
    ChannelJoin,
    #[serde(rename = "cl")]
    ChannelLeave,
    #[serde(rename = "cot")]
    ChannelOwnershipTransfer,
    #[serde(rename = "csr")]
    ChannelSessionKeyRotate,
    #[serde(rename = "ciu")]
    ChannelInfoUpdate,
    #[serde(rename = "cru")]
    ChannelMembersRoleUpdate,
    #[serde(rename = "cmb")]
    ChannelMembersBan,
    #[serde(rename = "cmu")]
    ChannelMembersUnban,
    #[serde(rename = "cmr")]
    ChannelMembersRemove,
    #[serde(rename = "cmj")]
    ChannelMemberJoin,
    #[serde(rename = "cml")]
    ChannelMemberLeave,
}

impl NotificationEvent {
    pub(crate) fn value(self) -> i32 { self as i32 }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SessionNew => "sn",
            Self::ContactSync => "cs",
            Self::ChannelCreate => "cc",
            Self::ChannelDelete => "cd",
            Self::ChannelJoin => "cj",
            Self::ChannelLeave => "cl",
            Self::ChannelOwnershipTransfer => "cot",
            Self::ChannelSessionKeyRotate => "csr",
            Self::ChannelInfoUpdate => "ciu",
            Self::ChannelMembersRoleUpdate => "cru",
            Self::ChannelMembersBan => "cmb",
            Self::ChannelMembersUnban => "cmu",
            Self::ChannelMembersRemove => "cmr",
            Self::ChannelMemberJoin => "cmj",
            Self::ChannelMemberLeave => "cml",
        }
    }
}

impl TryFrom<i32> for NotificationEvent {
    type Error = crate::errors::Error;

    fn try_from(value: i32) -> Result<Self> {
        use NotificationEvent::*;
        [SessionNew, ContactSync, ChannelCreate, ChannelDelete, ChannelJoin, ChannelLeave,
            ChannelOwnershipTransfer, ChannelSessionKeyRotate, ChannelInfoUpdate, ChannelMembersRoleUpdate,
            ChannelMembersBan, ChannelMembersUnban, ChannelMembersRemove, ChannelMemberJoin, ChannelMemberLeave]
            .into_iter().find(|event| event.value() == value)
            .ok_or_else(|| EncodingError::new(format!("Invalid notification event: {value}")).into())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Notification {
    pub(crate) id: Id,
    #[serde(rename = "s")]
    pub(crate) source: Id,
    #[serde(rename = "t")]
    pub(crate) timestamp: i64,
    #[serde(rename = "e")]
    pub(crate) event: NotificationEvent,
    #[serde(rename = "b", default, skip_serializing_if = "Option::is_none")]
    pub(crate) body: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

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

impl Notification {
    fn validate(&self) -> Result<()> {
        if self.timestamp < 0 {
            return Err(EncodingError::new("Notification timestamp is negative"));
        }
        match (self.event, self.body.as_ref()) {
            (NotificationEvent::ChannelDelete | NotificationEvent::ChannelLeave, None | Some(Value::Null)) => Ok(()),
            (NotificationEvent::ChannelOwnershipTransfer | NotificationEvent::ChannelMemberLeave, Some(Value::Bytes(id)))
                if id.len() == 32 => Ok(()),
            (NotificationEvent::ChannelSessionKeyRotate, Some(Value::Bytes(_))) => Ok(()),
            (NotificationEvent::ChannelMembersBan | NotificationEvent::ChannelMembersUnban
                | NotificationEvent::ChannelMembersRemove, Some(Value::Array(_))) => Ok(()),
            (NotificationEvent::SessionNew | NotificationEvent::ContactSync
                | NotificationEvent::ChannelCreate | NotificationEvent::ChannelJoin
                | NotificationEvent::ChannelInfoUpdate | NotificationEvent::ChannelMembersRoleUpdate
                | NotificationEvent::ChannelMemberJoin, Some(Value::Map(_))) => Ok(()),
            _ => Err(EncodingError::new("Notification body does not match its event")),
        }
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding notification failed: {error}")))?)
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let notification: Self = serde_cbor::from_slice(bytes)
            .map_err(|error| EncodingError::new(format!("Malformed notification: {error}")))?;
        notification.validate()?;
        Ok(notification)
    }
}
