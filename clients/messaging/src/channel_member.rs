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

use boson_api::{Id};
use crate::channel::Role;
use serde::{Serialize, Deserialize};
pub(crate) use crate::internal::codec;

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelMember {
    id: Id,
    #[serde(rename = "r", with = "codec::role")]
    role: Role,
    #[serde(rename = "j")]
    joined: u64,
}

/// A single member of a channel, combining identity and role.
impl ChannelMember {
    pub(crate) fn new(id: Id, role: Role, joined: u64) -> Self {
        Self { id, role, joined }
    }

    pub(crate) fn id(&self) -> &Id {
        &self.id
    }

    pub(crate) fn role(&self) -> Role {
        self.role
    }

    pub(crate) fn joined(&self) -> u64 {
        self.joined
    }

    pub(crate) fn is_owner(&self) -> bool {
        self.role == Role::Owner
    }

    pub(crate) fn is_moderator(&self) -> bool {
        self.role == Role::Moderator
    }

    pub(crate) fn is_member(&self) -> bool {
        self.role == Role::Member
    }

    pub(crate) fn is_banned(&self) -> bool {
        self.role == Role::Banned
    }
}

#[cfg(test)]
#[path = "unitests/test_channel_member.rs"]
mod test_channel_member;
