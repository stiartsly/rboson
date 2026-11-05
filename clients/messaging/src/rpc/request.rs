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

use boson_api::{utils, Id};
use crate::{
    channel::{Permission, Role},
    internal::{
        ChannelSessionKeyRotation,
        ContactMutation,
        NewChannelInfo,
        ChannelMembersRole,
    },
};
use super::method::Method;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum Params {
    ContactSync(i32),
    RevokeSession(Id),
    SetChannelOwner(Id),
    ContactMutate(ContactMutation),
    CreateChannel(NewChannelInfo),
    JoinChannel(JoinChannel),
    RotateChannelSessionKey(ChannelSessionKeyRotation),
    UpdateChannelInfo(ChannelInfoUpdate),
    ChannelMembersRole(ChannelMembersRole),
    BanChannelMembers(Vec<Id>),
    UnbanChannelMembers(Vec<Id>),
    RemoveChannelMembers(Vec<Id>),
}

#[derive(Debug, Serialize)]
pub(crate) struct JoinChannel {
    #[serde(rename = "c", with = "utils::id")]
    channel_id: Id,
    #[serde(rename = "sid", with = "utils::id")]
    session_id: Id,
    #[serde(rename = "i", with = "utils::id")]
    inviter: Id,
    #[serde(rename = "e")]
    expiration_ms: u64,
    #[serde(rename = "sig", with = "utils::bytes")]
    signature: Vec<u8>,
    #[serde(rename = "sk", with = "utils::bytes")]
    session_key: Vec<u8>,
    #[serde(rename = "ie", with = "utils::id_opt")]
    invitee: Option<Id>,
}

impl JoinChannel {
    pub(crate) fn new(
        channel_id: Id,
        session_id: Id,
        inviter: Id,
        expiration_ms: u64,
        signature: Vec<u8>,
        session_key: Vec<u8>,
        invitee: Option<Id>,
    ) -> Self {
        Self {
            channel_id,
            session_id,
            inviter,
            expiration_ms,
            signature,
            session_key,
            invitee,
        }
    }
}

#[derive(Debug, Default,Serialize)]
pub(crate) struct ChannelInfoUpdate {
    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    permission: Option<i32>,
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    name: Option<Option<String>>,
    #[serde(rename = "nt", skip_serializing_if = "Option::is_none")]
    notice: Option<Option<String>>,
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")]
    announced: Option<bool>,
}

impl ChannelInfoUpdate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn permission(mut self, permission: Permission) -> Self {
        self.permission = Some(permission as i32);
        self
    }

    pub(crate) fn name(mut self, name: Option<String>) -> Self {
        self.name = Some(name);
        self
    }

    pub(crate) fn notice(mut self, notice: Option<String>) -> Self {
        self.notice = Some(notice);
        self
    }

    pub(crate) fn announced(mut self, announced: bool) -> Self {
        self.announced = Some(announced);
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.permission.is_none()
            && self.name.is_none()
            && self.notice.is_none()
            && self.announced.is_none()
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RPCRequest {
    #[serde(rename = "id")]
    id: i64,

    #[serde(rename = "m")]
    method: Method,

    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    params: Option<Params>,

    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    cookie: Option<Value>,
}

impl RPCRequest {
    fn new(id: i64, method: Method, params: Option<Params>) -> Self {
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

    pub(crate) fn with_cookie(mut self, cookie: Vec<u8>) -> Self {
        self.cookie = Some(Value::Bytes(cookie));
        self
    }

    pub(crate) fn method(&self) -> Method {
        self.method
    }

    pub(crate) fn list_sessions(id: i64) -> Self {
        Self::new(id, Method::SessionList, None)
    }

    pub(crate) fn revoke_session(id: i64, device_id: Id) -> Self {
        let params = Params::RevokeSession(device_id);
        Self::new(id, Method::SessionRevoke, Some(params))
    }

    pub(crate) fn contact_mutate(id: i64, mutation: ContactMutation) -> Self {
        let params = Params::ContactMutate(mutation);
        Self::new(id, Method::ContactMutate, Some(params))
    }

    pub(crate) fn contact_sync(id: i64, revision: i32) -> Self {
        let params = Params::ContactSync(revision);
        Self::new(id, Method::ContactSync, Some(params))
    }

    pub(crate) fn create_channel(id: i64, info: NewChannelInfo) -> Self {
        let params = Params::CreateChannel(info);
        Self::new(id, Method::ChannelCreate, Some(params))
    }

    pub(crate) fn join_channel(id: i64, params: JoinChannel) -> Self {
        let params = Params::JoinChannel(params);
        Self::new(id, Method::ChannelJoin, Some(params))
    }

    pub(crate) fn delete_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelDelete, None)
    }

    pub(crate) fn leave_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelLeave, None)
    }

    pub(crate) fn get_channel_info(id: i64) -> Self {
        Self::new(id, Method::ChannelInfo, None)
    }

    pub(crate) fn transfer_channel_ownership(id: i64, new_owner: Id) -> Self {
        let params = Params::SetChannelOwner(new_owner);
        Self::new(id, Method::ChannelOwnershipTransfer, Some(params))
    }

    pub(crate) fn rotate_channel_session_key(id: i64, rotation: ChannelSessionKeyRotation) -> Self {
        let params = Params::RotateChannelSessionKey(rotation);
        Self::new(id, Method::ChannelSessionKeyRotate, Some(params))
    }

    pub(crate) fn update_channel_info(id: i64, changes: ChannelInfoUpdate) -> Self {
        let params = Params::UpdateChannelInfo(changes);
        Self::new(id, Method::ChannelInfoUpdate, Some(params))
    }

    pub(crate) fn update_channel_members_role(id: i64, members: Vec<Id>, role: Role) -> Self {
        let params = Params::ChannelMembersRole(ChannelMembersRole::new(members, role));
        Self::new(id, Method::ChannelMembersRoleUpdate, Some(params))
    }

    pub(crate) fn ban_channel_members(id: i64, members: Vec<Id>) -> Self {
        let params = Params::BanChannelMembers(members);
        Self::new(id, Method::ChannelMembersBan, Some(params))
    }

    pub(crate) fn unban_channel_members(id: i64, members: Vec<Id>) -> Self {
        let params = Params::UnbanChannelMembers(members);
        Self::new(id, Method::ChannelMembersUnban, Some(params))
    }

    pub(crate) fn remove_channel_members(id: i64, members: Vec<Id>) -> Self {
        let params = Params::RemoveChannelMembers(members);
        Self::new(id, Method::ChannelMembersRemove, Some(params))
    }
}
