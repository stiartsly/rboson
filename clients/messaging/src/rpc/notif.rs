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

use serde::{Deserialize, Serialize};
use serde_cbor::{self, value};

use boson_api::{
    Id,
    Error,
    Result,
};
use crate::{
    channel,
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
