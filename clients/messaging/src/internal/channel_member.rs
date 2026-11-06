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

use std::time::SystemTime;
use serde::{Serialize, Deserialize};

use boson_api::Id;
use crate::channel::Role;
use crate::interfaces::ChannelMemberOps;
use crate::internal::serde_utils::role;

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhotonChannelMember {
    id: Id,
    #[serde(rename = "r", with = "role")]
    role: i32,
    #[serde(rename = "j")]
    joined: u64,
}

impl PhotonChannelMember {
    pub fn new(id: Id, role: Role, joined: u64) -> Self {
        Self {
            id,
            role: role as i32,
            joined
        }
    }
}

impl ChannelMemberOps for PhotonChannelMember {
    fn id(&self) -> &Id {
        &self.id
    }

    fn role(&self) -> Role {
        Role::from_i32(self.role)
    }

    fn joined(&self) -> SystemTime {
        to_system_time(self.joined)
    }

    fn clone_box(&self) -> Box<dyn crate::ChannelMemberOps> {
        Box::new(self.clone())
    }
}

#[inline]
fn to_system_time(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
}

#[cfg(test)]
#[path = "../unitests/test_channel_member.rs"]
mod test_channel_member;
