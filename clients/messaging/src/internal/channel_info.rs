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
use serde::{Serialize, Deserialize};

use boson_api::{
    utils as api_utils,
    Id,
};
use crate::{
    channel::Permission,
    internal::utils as internal_utils,
    ChannelMember,
};

/// Channel metadata and membership as sent by the service.
#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ChannelInfo {
    #[serde(rename = "id", with ="api_utils::id")]
    channel_id: Id,
    #[serde(rename = "o", with ="api_utils::id")]
    owner_id: Id,
    #[serde(rename = "sid", with ="api_utils::id")]
    session_id: Id,

    #[serde(
        rename = "sk",
        default,
        with = "api_utils::bytes_opt"
    )]
    session_key: Option<Vec<u8>>,

    #[serde(
        rename = "p",
        with = "internal_utils::permission"
    )]
    permission: Permission,

    #[serde(
        rename = "n",
        default
    )]
    name: Option<String>,

    #[serde(
        rename = "nt",
        default,
        skip_serializing_if = "api_utils::is_default"
    )]
    notice: Option<String>,

    #[serde(
        rename = "a",
        default,
        skip_serializing_if = "api_utils::is_default"
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
        as_systemtime(self.created_at)
    }

    pub(crate) fn updated_at(&self) -> SystemTime {
        as_systemtime(self.updated_at)
    }

    pub(crate) fn members(&self) -> &[ChannelMember] {
        &self.members
    }
}

fn as_systemtime(millis: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(millis)
}

#[cfg(test)]
#[path = "../unitests/test_channel_info.rs"]
mod test_channel_info;
