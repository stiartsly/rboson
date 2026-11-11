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

use serde::Serialize;
use serde_cbor::Value;

use boson_api::Id;
use crate::channel::Role;
use crate::internal::{
    ChannelSessionKeyRotation,
    ContactMutation,
    NewChannelInfo,
    ChannelMembersRole,
    ChannelInfoUpdate,
    JoinChannel,
};
use super::method::Method;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum Params {
    SessionList(()),
    SessionRevoke(Id),
    ContactMutate(ContactMutation),
    ContactSync(i32),
    ChannelCreate(NewChannelInfo),
    ChannelDelete(()),
    ChannelOwnershipTransfer(Id),
    ChannelSessionKeyRotate(ChannelSessionKeyRotation),
    ChannelInfoUpdate(ChannelInfoUpdate),
    ChannelMembersRoleUpdate(ChannelMembersRole),
    ChannelMembersBan(Vec<Id>),
    ChannelMembersUnban(Vec<Id>),
    ChannelMembersRemove(Vec<Id>),
    ChannelJoin(JoinChannel),
    ChannelLeave(()),
    ChannelInfo(()),
}

#[derive(Debug, Serialize)]
pub(crate) struct RPCRequest {
    #[serde(rename = "id")]
    id: i64,

    #[serde(rename = "m")]
    method: Method,

    #[serde(rename = "p")]
    params: Params,

    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    cookie: Option<Value>,
}

impl RPCRequest {
    fn new(id: i64, method: Method, params: Params) -> Self {
        Self {
            id,
            method,
            params,
            cookie: None,
        }
    }

    pub(crate) fn with_id(mut self, id: i64) -> Self {
        self.id = id;
        self
    }

    pub(crate) fn _with_cookie(mut self, cookie: Vec<u8>) -> Self {
        self.cookie = Some(Value::Bytes(cookie));
        self
    }

    pub(crate) fn method(&self) -> Method {
        self.method
    }

    pub(crate) fn list_sessions(id: i64) -> Self {
        Self::new(id, Method::SessionList, Params::SessionList(()))
    }

    pub(crate) fn revoke_session(id: i64, device_id: Id) -> Self {
        let params = Params::SessionRevoke(device_id);
        Self::new(id, Method::SessionRevoke, params)
    }

    pub(crate) fn contact_mutate(id: i64, mutation: ContactMutation) -> Self {
        let params = Params::ContactMutate(mutation);
        Self::new(id, Method::ContactMutate, params)
    }

    pub(crate) fn contact_sync(id: i64, revision: i32) -> Self {
        let params = Params::ContactSync(revision);
        Self::new(id, Method::ContactSync, params)
    }

    pub(crate) fn create_channel(id: i64, info: NewChannelInfo) -> Self {
        let params = Params::ChannelCreate(info);
        Self::new(id, Method::ChannelCreate, params)
    }

    pub(crate) fn join_channel(id: i64, params: JoinChannel) -> Self {
        let params = Params::ChannelJoin(params);
        Self::new(id, Method::ChannelJoin, params)
    }

    pub(crate) fn delete_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelDelete, Params::ChannelDelete(()))
    }

    pub(crate) fn leave_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelLeave, Params::ChannelLeave(()))
    }

    pub(crate) fn get_channel_info(id: i64) -> Self {
        Self::new(id, Method::ChannelInfo, Params::ChannelInfo(()))
    }

    pub(crate) fn transfer_channel_ownership(id: i64, new_owner: Id) -> Self {
        let params = Params::ChannelOwnershipTransfer(new_owner);
        Self::new(id, Method::ChannelOwnershipTransfer, params)
    }

    pub(crate) fn rotate_channel_session_key(id: i64, rotation: ChannelSessionKeyRotation) -> Self {
        let params = Params::ChannelSessionKeyRotate(rotation);
        Self::new(id, Method::ChannelSessionKeyRotate, params)
    }

    pub(crate) fn update_channel_info(id: i64, changes: ChannelInfoUpdate) -> Self {
        let params = Params::ChannelInfoUpdate(changes);
        Self::new(id, Method::ChannelInfoUpdate, params)
    }

    pub(crate) fn update_channel_members_role(id: i64, members: Vec<Id>, role: Role) -> Self {
        let params = Params::ChannelMembersRoleUpdate(ChannelMembersRole::new(members, role));
        Self::new(id, Method::ChannelMembersRoleUpdate, params)
    }

    pub(crate) fn ban_channel_members(id: i64, members: Vec<Id>) -> Self {
        let params = Params::ChannelMembersBan(members);
        Self::new(id, Method::ChannelMembersBan, params)
    }

    pub(crate) fn unban_channel_members(id: i64, members: Vec<Id>) -> Self {
        let params = Params::ChannelMembersUnban(members);
        Self::new(id, Method::ChannelMembersUnban, params)
    }

    pub(crate) fn remove_channel_members(id: i64, members: Vec<Id>) -> Self {
        let params = Params::ChannelMembersRemove(members);
        Self::new(id, Method::ChannelMembersRemove, params)
    }
}
