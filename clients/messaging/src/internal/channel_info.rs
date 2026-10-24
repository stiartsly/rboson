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

use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};

use crate::channel::Permission;
use crate::internal::codec;
use boson_api::{utils, Id};

use super::ChannelMember;

/// Channel metadata and membership as sent by the service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ChannelInfo {
    #[serde(rename = "id")]
    channel_id: Id,
    #[serde(rename = "o")]
    owner_id: Id,
    #[serde(rename = "sid")]
    session_id: Id,

    #[serde(
        rename = "sk",
        default,
        serialize_with = "boson_api::utils::codec::bytes_opt::serialize",
        deserialize_with = "boson_api::utils::codec::bytes_opt::deserialize",
        skip_serializing_if = "is_bytes_none_or_empty"
    )]
    session_key: Option<Vec<u8>>,

    #[serde(rename = "p", with = "codec::permission")]
    permission: Permission,

    #[serde(
        rename = "n",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    name: Option<String>,

    #[serde(
        rename = "nt",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    notice: Option<String>,

    #[serde(
        rename = "a",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    announce: bool,

    #[serde(rename = "c", default)]
    created_at: u64,

    #[serde(rename = "u", default)]
    updated_at: u64,

    #[serde(
        rename = "m",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    members: Vec<ChannelMember>,
}

impl ChannelInfo {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        channel_id: Id,
        owner_id: Id,
        session_id: Id,
        session_key: Option<Vec<u8>>,
        permission: Permission,
        name: Option<String>,
        notice: Option<String>,
        announce: bool,
        created_at: u64,
        updated_at: u64,
        members: Vec<ChannelMember>,
    ) -> Self {
        Self {
            channel_id,
            owner_id,
            session_id,
            session_key,
            permission,
            name,
            notice,
            announce,
            created_at,
            updated_at,
            members,
        }
    }

    pub(crate) fn channel_id(&self) -> &Id {
        &self.channel_id
    }

    pub(crate) fn owner_id(&self) -> &Id {
        &self.owner_id
    }

    pub(crate) fn session_id(&self) -> &Id {
        &self.session_id
    }

    pub(crate) fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    pub(crate) fn permission(&self) -> Permission {
        self.permission
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub(crate) fn announce(&self) -> bool {
        self.announce
    }

    pub(crate) fn created_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.created_at)
    }

    pub(crate) fn updated_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.updated_at)
    }

    pub(crate) fn members(&self) -> &[ChannelMember] {
        &self.members
    }
}

fn is_bytes_none_or_empty(value: &Option<Vec<u8>>) -> bool {
    value.as_ref().map_or(true, Vec::is_empty)
}

fn is_none_or_empty(value: &Option<String>) -> bool {
    value.as_deref().map_or(true, str::is_empty)
}

#[cfg(test)]
#[path = "../unitests/test_channel_info.rs"]
mod test_channel_info;
