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
use serde::{Serialize, Deserialize};
use crate::Permission;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct ChannelInfoUpdate {
    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    permission: Option<i32>,
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    name: Option<Option<String>>,
    #[serde(rename = "nt", skip_serializing_if = "Option::is_none")]
    notice: Option<Option<String>>,
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")]
    announce: Option<bool>,
}

impl ChannelInfoUpdate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set_permission(mut self, permission: Permission) -> Self {
        self.permission = Some(permission as i32);
        self
    }

    pub(crate) fn set_name(mut self, name: Option<String>) -> Self {
        self.name = Some(name);
        self
    }

    pub(crate) fn set_notice(mut self, notice: Option<String>) -> Self {
        self.notice = Some(notice);
        self
    }

    pub(crate) fn set_announce(mut self, announce: bool) -> Self {
        self.announce = Some(announce);
        self
    }

    pub(crate) fn permission(&self) -> Option<Permission> {
        self.permission.map(Permission::from_i32)
    }

    pub(crate) fn name(&self) -> Option<Option<&str>> {
        self.name.as_ref().map(|name| name.as_deref())
    }

    pub(crate) fn notice(&self) -> Option<Option<&str>> {
        self.notice.as_ref().map(|notice| notice.as_deref())
    }

    pub(crate) fn announce(&self) -> Option<bool> {
        self.announce
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.permission.is_none()
            && self.name.is_none()
            && self.notice.is_none()
            && self.announce.is_none()
    }
}

impl fmt::Display for ChannelInfoUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChannelInfoUpdate={{ ")?;
        if let Some(permission) = self.permission {
            write!(f, "permission: {:?}, ", permission)?;
        }
        if let Some(ref name) = self.name {
            write!(f, "name: {:?}, ", name)?;
        }
        if let Some(ref notice) = self.notice {
            write!(f, "notice: {:?}, ", notice)?;
        }
        if let Some(ref announce) = self.announce {
            write!(f, "announce: {:?}, ", announce)?;
        }
        write!(f, "}}")
    }
}
