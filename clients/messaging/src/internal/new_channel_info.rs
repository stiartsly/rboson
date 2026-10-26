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

use std::fmt;
use serde::Serialize;

use crate::internal::codec;
use crate::Permission;
use boson_api::{utils, Id};

/// Channel creation request sent to the service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct NewChannelInfo {
    #[serde(
        rename = "sid",
        with = "utils::codec::id"
    )]
    session_id: Id,

    #[serde(
        rename = "sk",
        with = "utils::codec::bytes"
    )]
    session_key: Vec<u8>,

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
        skip_serializing_if = "Option::is_none"
    )]
    notice: Option<String>,

    #[serde(
        rename = "a",
        skip_serializing_if = "utils::is_default"
    )]
    announce: bool,
}

impl NewChannelInfo {
    pub(crate) fn new(
        session_id: Id,
        session_key: Vec<u8>,
        permission: Permission,
        name: Option<String>,
        notice: Option<String>,
        announce: bool,
    ) -> Self {
        Self {
            session_id,
            session_key,
            permission,
            name,
            notice,
            announce,
        }
    }
}

impl fmt::Display for NewChannelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewChannelInfo")
            .field("session_id", &self.session_id)
            .field("session_key_len", &self.session_key.len())
            .field("permission", &self.permission)
            .field("name", &self.name)
            .field("notice", &self.notice)
            .field("announce", &self.announce)
            .finish()
    }
}
