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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub(crate) enum Method {
    #[serde(rename = "sl")] SessionList = 1,
    #[serde(rename = "sr")] SessionRevoke,
    #[serde(rename = "cm")] ContactMutate,
    #[serde(rename = "cs")] ContactSync,
    #[serde(rename = "cc")] ChannelCreate,
    #[serde(rename = "cd")] ChannelDelete,
    #[serde(rename = "cot")] ChannelOwnershipTransfer,
    #[serde(rename = "csr")] ChannelSessionKeyRotate,
    #[serde(rename = "ciu")] ChannelInfoUpdate,
    #[serde(rename = "cru")] ChannelMembersRoleUpdate,
    #[serde(rename = "cmb")] ChannelMembersBan,
    #[serde(rename = "cmu")] ChannelMembersUnban,
    #[serde(rename = "cmr")] ChannelMembersRemove,
    #[serde(rename = "cj")] ChannelJoin,
    #[serde(rename = "cl")] ChannelLeave,
    #[serde(rename = "ci")] ChannelInfo,
}

impl Method {
    pub(crate) fn value(self) -> i32 {
        self as i32
    }

    pub(crate) fn is_service_rpc(self) -> bool {
        self.value() <= Self::ChannelCreate.value()
    }

    pub(crate) fn is_channel_rpc(self) -> bool {
        (Self::ChannelDelete.value()..=Self::ChannelInfo.value()).contains(&self.value())
    }

    pub(crate) fn is_owner_privileged_channel_rpc(self) -> bool {
        (Self::ChannelDelete.value()..=Self::ChannelInfoUpdate.value()).contains(&self.value())
    }

    pub(crate) fn is_moderator_privileged_channel_rpc(self) -> bool {
        (Self::ChannelMembersRoleUpdate.value()..=Self::ChannelMembersRemove.value()).contains(&self.value())
    }

    pub(crate) fn is_unprivileged_channel_rpc(self) -> bool {
        (Self::ChannelJoin.value()..=Self::ChannelInfo.value()).contains(&self.value())
    }
}

impl TryFrom<i32> for Method {
    type Error = boson_api::Error;

    fn try_from(value: i32) -> boson_api::Result<Self> {
        use Method::*;
        [
            SessionList, SessionRevoke, ContactMutate, ContactSync, ChannelCreate,
            ChannelDelete, ChannelOwnershipTransfer, ChannelSessionKeyRotate,
            ChannelInfoUpdate, ChannelMembersRoleUpdate, ChannelMembersBan,
            ChannelMembersUnban, ChannelMembersRemove, ChannelJoin, ChannelLeave, ChannelInfo,
        ]
        .into_iter()
        .find(|method| method.value() == value)
        .ok_or_else(|| boson_api::errors::ProtocolError::new(format!("Invalid RPC method: {value}")).into())
    }
}
