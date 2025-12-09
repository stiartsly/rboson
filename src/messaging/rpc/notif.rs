use serde::{Deserialize, Serialize};
use serde_cbor::{self, value};

use crate::{
    Id,
    Error,
    error::Result,
    messaging::channel,
};

pub(crate) mod events {
    pub const USER_PROFILE: u32             = 1;
    pub const CHANNEL_PROFILE: u32          = 2; // owner, permission, name, notice
    pub const CHANNEL_DELETED: u32          = 3;
    pub const CHANNEL_MEMBER_JOINED: u32    = 4;
    pub const CHANNEL_MEMBER_LEFT: u32      = 5;
    pub const CHANNEL_MEMBERS_ROLE: u32     = 6;
    pub const CHANNEL_MEMBERS_BANNED: u32   = 7;
    pub const CHANNEL_MEMBERS_UNBANNED: u32 = 8;
    pub const CHANNEL_MEMBERS_REMOVED: u32  = 9;
}

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Notification {
    #[serde(rename = "e")]
    event: u32,

    #[serde(rename = "o")]
    operator: Id,

    #[serde(rename = "d", skip_serializing_if = "Option::is_none")]
    data: Option<value::Value>,
}

#[allow(unused)]
impl Notification {
    pub(crate) fn from(body: &[u8]) -> Result<Self> {
        serde_cbor::from_slice::<Notification>(body).map_err(|e|
            Error::Protocol(format!("Error parsing notification: {}", e))
        )
    }

    pub(crate) fn event(&self) -> u32 {
        self.event
    }

    pub(crate) fn operator(&self) -> &Id {
        &self.operator
    }

    pub(crate) fn data_ref(&self) -> Option<&value::Value> {
        self.data.as_ref()
    }

    pub(crate) fn data<T>(&mut self) -> Result<T> where T: serde::de::DeserializeOwned {
        let Some(v) = self.data.take() else {
            return Err(Error::Protocol("Missing data in notification".into()))
        };
        return serde_cbor::value::from_value(v).map_err(|e|
            Error::Protocol(format!("Internal error: bad RPC response with error {e}"))
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChannelMembersRoleUpdated {
    #[serde(rename = "r")]
    role: channel::Role,

    #[serde(rename = "id")]
    members: Vec<Id>,
}

impl ChannelMembersRoleUpdated {
    pub(crate) fn role(&self) -> channel::Role {
        self.role
    }

    pub(crate) fn members(&self) -> &[Id] {
        &self.members
    }
}
