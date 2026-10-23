use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

use crate::errors::EncodingError;
use crate::SessionInfo;
use boson_api::{
    errors::{Error, Result},
    Id,
};

use super::channel::{ChannelInfo, ChannelMember, ChannelMembersRole};
use super::contact::ContactSync;

/// Notification event tag used on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub(crate) enum Event {
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

impl Event {
    pub(crate) const fn as_str(self) -> &'static str {
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

impl TryFrom<i32> for Event {
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
            _ => Err(EncodingError::new(format!(
                "Invalid notification event: {value}"
            ))
            .into()),
        }
    }
}

/// Payload carried by a [`Notification`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "e", content = "b")]
pub(crate) enum Body {
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
    ChannelSessionKeyRotate(
        #[serde(with = "serde_with::As::<serde_with::Bytes>")] Vec<u8>,
    ),
    #[serde(rename = "ciu")]
    ChannelInfoUpdate(BTreeMap<serde_cbor::Value, serde_cbor::Value>),
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

impl Body {
    pub(crate) fn event(&self) -> Event {
        match self {
            Self::SessionNew(_) => Event::SessionNew,
            Self::ContactSync(_) => Event::ContactSync,
            Self::ChannelCreate(_) => Event::ChannelCreate,
            Self::ChannelDelete => Event::ChannelDelete,
            Self::ChannelJoin(_) => Event::ChannelJoin,
            Self::ChannelLeave => Event::ChannelLeave,
            Self::ChannelOwnershipTransfer(_) => Event::ChannelOwnershipTransfer,
            Self::ChannelSessionKeyRotate(_) => Event::ChannelSessionKeyRotate,
            Self::ChannelInfoUpdate(_) => Event::ChannelInfoUpdate,
            Self::ChannelMembersRoleUpdate(_) => Event::ChannelMembersRoleUpdate,
            Self::ChannelMembersBan(_) => Event::ChannelMembersBan,
            Self::ChannelMembersUnban(_) => Event::ChannelMembersUnban,
            Self::ChannelMembersRemove(_) => Event::ChannelMembersRemove,
            Self::ChannelMemberJoin(_) => Event::ChannelMemberJoin,
            Self::ChannelMemberLeave(_) => Event::ChannelMemberLeave,
        }
    }
}

/// A home-peer notification envelope.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Notification {
    #[serde(rename = "id")]
    id: Id,
    #[serde(rename = "s")]
    source: Id,
    #[serde(rename = "t")]
    timestamp: u64,
    #[serde(flatten)]
    body: Body,
}

impl Notification {
    pub(crate) fn id(&self) -> Id {
        self.id
    }

    pub(crate) fn source(&self) -> Id {
        self.source
    }

    pub(crate) fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub(crate) fn body(&self) -> &Body {
        &self.body
    }
}

#[cfg(test)]
#[path = "../unitests/test_notification.rs"]
mod test_notification;
