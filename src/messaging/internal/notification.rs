use serde::{Deserialize, Serialize};
use crate::{
    Id,
    errors::{Error, Result},
};
use crate::messaging::errors::EncodingError;
use crate::messaging::SessionInfo;
use super::{ChannelInfo, ChannelMember, ChannelMembersRole, ContactSync};

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum NotificationEvent {
    #[serde(rename = "sn")]
    SessionNew = 1,
    #[serde(rename = "cs")]
    ContactSync = 2,
    #[serde(rename = "cc")]
    ChannelCreate = 21,
    #[serde(rename = "cd")]
    ChannelDelete = 22,
    #[serde(rename = "cj")]
    ChannelJoin = 23,
    #[serde(rename = "cl")]
    ChannelLeave = 24,
    #[serde(rename = "cot")]
    ChannelOwnershipTransfer = 25,
    #[serde(rename = "csr")]
    ChannelSessionKeyRotate = 26,
    #[serde(rename = "ciu")]
    ChannelInfoUpdate = 27,
    #[serde(rename = "cru")]
    ChannelMembersRoleUpdate = 28,
    #[serde(rename = "cmb")]
    ChannelMembersBan = 29,
    #[serde(rename = "cmu")]
    ChannelMembersUnban = 30,
    #[serde(rename = "cmr")]
    ChannelMembersRemove = 31,
    #[serde(rename = "cmj")]
    ChannelMemberJoin = 32,
    #[serde(rename = "cml")]
    ChannelMemberLeave = 33,
}

impl NotificationEvent {
    pub(crate) fn value(self) -> i32 {
        self as i32
    }

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
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            1 => Ok(Self::SessionNew),
            2 => Ok(Self::ContactSync),
            21 => Ok(Self::ChannelCreate),
            22 => Ok(Self::ChannelDelete),
            23 => Ok(Self::ChannelJoin),
            24 => Ok(Self::ChannelLeave),
            25 => Ok(Self::ChannelOwnershipTransfer),
            26 => Ok(Self::ChannelSessionKeyRotate),
            27 => Ok(Self::ChannelInfoUpdate),
            28 => Ok(Self::ChannelMembersRoleUpdate),
            29 => Ok(Self::ChannelMembersBan),
            30 => Ok(Self::ChannelMembersUnban),
            31 => Ok(Self::ChannelMembersRemove),
            32 => Ok(Self::ChannelMemberJoin),
            33 => Ok(Self::ChannelMemberLeave),
            _ => Err(EncodingError::new(format!("Invalid notification event: {value}")).into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "e", content = "b")]
pub(crate) enum NotificationBody {
    #[serde(rename = "sn")]
    SessionNew(SessionInfo),
    #[serde(rename = "cs")]
    ContactSync(ContactSync),
    #[serde(rename = "cc")]
    ChannelCreate(ChannelInfo),
    #[serde(rename = "cd")]
    ChannelDelete,
    #[serde(rename = "cj")]
    ChannelJoin(ChannelInfo),
    #[serde(rename = "cl")]
    ChannelLeave,
    #[serde(rename = "cot")]
    ChannelOwnershipTransfer(Id),
    #[serde(rename = "csr")]
    ChannelSessionKeyRotate(#[serde(with = "serde_with::As::<serde_with::Bytes>")] Vec<u8>),
    #[serde(rename = "ciu")]
    ChannelInfoUpdate(std::collections::BTreeMap<serde_cbor::Value, serde_cbor::Value>),
    #[serde(rename = "cru")]
    ChannelMembersRoleUpdate(ChannelMembersRole),
    #[serde(rename = "cmb")]
    ChannelMembersBan(Vec<Id>),
    #[serde(rename = "cmu")]
    ChannelMembersUnban(Vec<Id>),
    #[serde(rename = "cmr")]
    ChannelMembersRemove(Vec<Id>),
    #[serde(rename = "cmj")]
    ChannelMemberJoin(ChannelMember),
    #[serde(rename = "cml")]
    ChannelMemberLeave(Id),
}

impl NotificationBody {
    pub(crate) fn event(&self) -> NotificationEvent {
        match self {
            Self::SessionNew(_) => NotificationEvent::SessionNew,
            Self::ContactSync(_) => NotificationEvent::ContactSync,
            Self::ChannelCreate(_) => NotificationEvent::ChannelCreate,
            Self::ChannelDelete => NotificationEvent::ChannelDelete,
            Self::ChannelJoin(_) => NotificationEvent::ChannelJoin,
            Self::ChannelLeave => NotificationEvent::ChannelLeave,
            Self::ChannelOwnershipTransfer(_) => NotificationEvent::ChannelOwnershipTransfer,
            Self::ChannelSessionKeyRotate(_) => NotificationEvent::ChannelSessionKeyRotate,
            Self::ChannelInfoUpdate(_) => NotificationEvent::ChannelInfoUpdate,
            Self::ChannelMembersRoleUpdate(_) => NotificationEvent::ChannelMembersRoleUpdate,
            Self::ChannelMembersBan(_) => NotificationEvent::ChannelMembersBan,
            Self::ChannelMembersUnban(_) => NotificationEvent::ChannelMembersUnban,
            Self::ChannelMembersRemove(_) => NotificationEvent::ChannelMembersRemove,
            Self::ChannelMemberJoin(_) => NotificationEvent::ChannelMemberJoin,
            Self::ChannelMemberLeave(_) => NotificationEvent::ChannelMemberLeave,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "NotificationData")]
pub(crate) struct Notification {
    #[serde(rename = "id")]
    pub(crate) id: Id,
    #[serde(rename = "s")]
    pub(crate) source: Id,
    #[serde(rename = "t")]
    pub(crate) timestamp: u64,
    #[serde(flatten)]
    pub(crate) body: NotificationBody,
}

#[derive(Deserialize)]
struct NotificationData {
    id: Id,
    #[serde(rename = "s")]
    source: Id,
    #[serde(rename = "t")]
    timestamp: u64,
    #[serde(flatten)]
    body: NotificationBody,
}

impl TryFrom<NotificationData> for Notification {
    type Error = Error;

    fn try_from(data: NotificationData) -> Result<Self> {
        i64::try_from(data.timestamp)
            .map_err(|_| EncodingError::new("Notification timestamp is out of range"))?;
        Ok(Self { id: data.id, source: data.source, timestamp: data.timestamp, body: data.body })
    }
}

#[cfg(test)]
#[path = "../unitests/test_notification.rs"]
mod test_notification;
