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

use boson_api::{utils, Id};
use crate::Permission;
use crate::internal::serde_utils::permission;

/// Channel creation request sent to the service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct NewChannelInfo {
    #[serde(rename = "sid", with = "utils::id")]
    session_id: Id,
    #[serde(rename = "sk", with = "utils::bytes")]
    session_key: Vec<u8>,
    #[serde(rename = "p", with = "permission")]
    permission: i32,
    #[serde(rename = "n",  skip_serializing_if = "utils::is_default")]
    name: Option<String>,
    #[serde(rename = "nt", skip_serializing_if = "utils::is_default")]
    notice: Option<String>,
    #[serde(rename = "a", skip_serializing_if = "utils::is_default")]
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
            permission: permission as i32,
            name,
            notice,
            announce,
        }
    }

    pub(crate) fn revised(&self, session_id: Id, session_key: Vec<u8>) -> Self {
        let mut revised = self.clone();
        revised.session_id = session_id;
        revised.session_key = session_key;
        revised
    }

    pub(crate) fn session_id(&self) -> Id {
        self.session_id
    }

    pub(crate) fn session_key(&self) -> &[u8] {
        &self.session_key
    }

    pub(crate) fn permission(&self) -> Permission {
        Permission::from_i32(self.permission)
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
