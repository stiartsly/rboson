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

use std::result::Result as StdResult;
use serde::{Serialize, Serializer};
use serde_cbor::Value;

use boson_api::{
    Id,
    Result
};

use crate::{
    channel::Role,
    errors::EncodingError,
    internal::NewChannelInfo
};
use super::{
    method::Method,
    params::{id_list, ChannelMembersRole, IdBytes},
};

#[derive(Debug)]
pub(crate) enum Params {
    ContactSync(i32),
    RevokeSession(Id),
    SetChannelOwner(Id),
    ContactMutate(Value),
    CreateChannel(NewChannelInfo),
    JoinChannel(Value),
    RotateChannelSessionKey(Value),
    UpdateChannelInfo(Value),
    ChannelMembersRole(ChannelMembersRole),
    BanChannelMembers(Vec<Id>),
    UnbanChannelMembers(Vec<Id>),
    RemoveChannelMembers(Vec<Id>),
}

impl Params {
    pub(crate) fn matches(&self, method: Method) -> bool {
        match (method, self) {
            (Method::SessionRevoke, Self::RevokeSession(_))
            | (Method::ChannelOwnershipTransfer, Self::SetChannelOwner(_))
            | (Method::ChannelCreate, Self::CreateChannel(_))
            | (Method::ChannelJoin, Self::JoinChannel(Value::Map(_)))
            | (Method::ChannelMembersRoleUpdate, Self::ChannelMembersRole(_)) => true,
            (Method::ContactSync, Self::ContactSync(revision)) => *revision >= 0,
            (Method::ContactMutate, Self::ContactMutate(Value::Map(_)))
            | (Method::ChannelSessionKeyRotate, Self::RotateChannelSessionKey(Value::Map(_)))
            | (Method::ChannelInfoUpdate, Self::UpdateChannelInfo(Value::Map(_))) => true,
            (Method::ChannelMembersBan, Self::BanChannelMembers(ids))
            | (Method::ChannelMembersUnban, Self::UnbanChannelMembers(ids))
            | (Method::ChannelMembersRemove, Self::RemoveChannelMembers(ids)) => !ids.is_empty(),
            _ => false,
        }
    }
}

impl Serialize for Params {
    fn serialize<S: Serializer>(&self, se: S) -> StdResult<S::Ok, S::Error> {
        match self {
            Self::ContactSync(revision) => revision.serialize(se),
            Self::RevokeSession(id) |
            Self::SetChannelOwner(id) => IdBytes(id).serialize(se),
            Self::CreateChannel(params) => params.serialize(se),
            Self::ContactMutate(value)
            | Self::JoinChannel(value)
            | Self::RotateChannelSessionKey(value)
            | Self::UpdateChannelInfo(value) => value.serialize(se),
            Self::ChannelMembersRole(params) => params.serialize(se),
            Self::BanChannelMembers(ids)
            | Self::UnbanChannelMembers(ids)
            | Self::RemoveChannelMembers(ids) => id_list::serialize(ids, se),
        }
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
        Self {id, method, params, cookie: None}
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

    pub(crate) fn contact_mutate(id: i64, mutation: Value) -> Self {
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

    pub(crate) fn join_channel(id: i64, ticket: Value) -> Self {
        let params = Params::JoinChannel(ticket);
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

    pub(crate) fn rotate_channel_session_key(id: i64, rotation: Value) -> Self {
        let params = Params::RotateChannelSessionKey(rotation);
        Self::new(id, Method::ChannelSessionKeyRotate, Some(params))
    }

    pub(crate) fn update_channel_info(id: i64, changes: Value) -> Self {
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

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_cbor::to_vec(self).map_err(|error| {
            EncodingError::new(format!("Encoding RPC request failed: {error}")).into()
        })
    }

    fn validate(&self) -> Result<()> {
        if self.id < 0 {
            return Err(EncodingError::new("Invalid RPC request ID"));
        }
        let valid = match (self.method, self.params.as_ref()) {
            (
                Method::SessionList
                | Method::ChannelDelete
                | Method::ChannelLeave
                | Method::ChannelInfo,
                None,
            ) => true,
            (method, Some(params)) => params.matches(method),
            _ => false,
        };
        if !valid {
            return Err(EncodingError::new("RPC parameters do not match the method"));
        }
        Ok(())
    }
}
