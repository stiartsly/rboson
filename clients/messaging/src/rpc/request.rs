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
use serde_cbor::Value;

use boson_api::{Id, Result};
use crate::errors::EncodingError;
use super::method::Method;

fn id_value(id: &Id) -> Value {
    Value::Bytes(id.as_bytes().to_vec())
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RPCRequest {
    #[serde(rename = "id")]
    id: i64,

    #[serde(rename = "m")]
    method: Method,

    #[serde(
        rename = "p",
        default,
        skip_serializing_if = "Option::is_none"
    )]
   params: Option<Value>,

    #[serde(
        rename = "c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    cookie: Option<Value>,
}

#[allow(dead_code)]
impl RPCRequest {
    fn new(id: i64, method: Method, params: Option<Value>) -> Self {
        Self { id, method, params, cookie: None }
    }

    pub(crate) fn method(&self) -> Method {
        self.method
    }

    pub(crate) fn list_sessions(id: i64) -> Self {
        Self::new(id, Method::SessionList, None)
    }

    pub(crate) fn revoke_session(id: i64, device_id: &Id) -> Self {
        Self::new(id, Method::SessionRevoke, Some(id_value(device_id)))
    }

    pub(crate) fn contact_mutate(id: i64, mutation: Value) -> Self {
        Self::new(id, Method::ContactMutate, Some(mutation))
    }

    pub(crate) fn contact_sync(id: i64, revision: i32) -> Self {
        Self::new(id, Method::ContactSync, Some(Value::Integer(revision.into())))
    }

    pub(crate) fn create_channel(id: i64, params: Value) -> Self {
        Self::new(id, Method::ChannelCreate, Some(params))
    }

    pub(crate) fn delete_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelDelete, None)
    }

    pub(crate) fn join_channel(id: i64, ticket: Value) -> Self {
        Self::new(id, Method::ChannelJoin, Some(ticket))
    }

    pub(crate) fn leave_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelLeave, None)
    }

    pub(crate) fn transfer_channel_ownership(id: i64, new_owner: &Id) -> Self {
        Self::new(id, Method::ChannelOwnershipTransfer, Some(id_value(new_owner)))
    }

    pub(crate) fn rotate_channel_session_key(id: i64, params: Value) -> Self {
        Self::new(id, Method::ChannelSessionKeyRotate, Some(params))
    }

    pub(crate) fn update_channel_info(id: i64, changes: Value) -> Self {
        Self::new(id, Method::ChannelInfoUpdate, Some(changes))
    }

    pub(crate) fn update_channel_members_role(id: i64, params: Value) -> Self {
        Self::new(id, Method::ChannelMembersRoleUpdate, Some(params))
    }

    pub(crate) fn ban_channel_members(id: i64, member_ids: &[Id]) -> Self {
        Self::new(id, Method::ChannelMembersBan, Some(id_list(member_ids)))
    }

    pub(crate) fn unban_channel_members(id: i64, member_ids: &[Id]) -> Self {
        Self::new(id, Method::ChannelMembersUnban, Some(id_list(member_ids)))
    }

    pub(crate) fn remove_channel_members(id: i64, member_ids: &[Id]) -> Self {
        Self::new(id, Method::ChannelMembersRemove, Some(id_list(member_ids)))
    }

    pub(crate) fn get_channel_info(id: i64) -> Self {
        Self::new(id, Method::ChannelInfo, None)
    }

    pub(crate) fn with_cookie(mut self, cookie: Vec<u8>) -> Self {
        self.cookie = Some(Value::Bytes(cookie));
        self
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding RPC request failed: {error}")).into())
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let request: Self = serde_cbor::from_slice(bytes)
            .map_err(|error| EncodingError::new(format!("Malformed RPC request: {error}")))?;
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<()> {
        if self.id < 0 || self.cookie.as_ref().is_some_and(|cookie| !matches!(cookie, Value::Bytes(_))) {
            return Err(EncodingError::new("Invalid RPC request ID or cookie"));
        }
        let valid = match (self.method, self.params.as_ref()) {
            (Method::SessionList | Method::ChannelDelete | Method::ChannelLeave | Method::ChannelInfo,
                None | Some(Value::Null)) => true,
            (Method::SessionRevoke | Method::ChannelOwnershipTransfer, Some(Value::Bytes(id))) => id.len() == 32,
            (Method::ContactSync, Some(Value::Integer(revision))) => i32::try_from(*revision).is_ok() && *revision >= 0,
            (Method::ContactMutate | Method::ChannelCreate | Method::ChannelJoin
                | Method::ChannelSessionKeyRotate | Method::ChannelInfoUpdate
                | Method::ChannelMembersRoleUpdate, Some(Value::Map(_))) => true,
            (Method::ChannelMembersBan | Method::ChannelMembersUnban | Method::ChannelMembersRemove,
                Some(Value::Array(ids))) => ids.iter().all(|id| matches!(id, Value::Bytes(id) if id.len() == 32)),
            _ => false,
        };
        if !valid {
            return Err(EncodingError::new("RPC parameters do not match the method"));
        }
        Ok(())
    }
}

fn id_list(ids: &[Id]) -> Value {
    Value::Array(ids.iter().map(id_value).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_request_uses_compact_method_and_round_trips() {
        assert_eq!(
            serde_cbor::value::to_value(Method::ChannelDelete).unwrap(),
            Value::Text("cd".to_string())
        );

        let request = RPCRequest::delete_channel(17);
        let parsed = RPCRequest::parse(&request.serialize().unwrap()).unwrap();

        assert_eq!(parsed.id, request.id);
        assert_eq!(parsed.method, request.method);
        assert_eq!(parsed.params, request.params);
    }

    #[test]
    fn typed_constructors_produce_valid_params() {
        let id = Id::random();
        for request in [
            RPCRequest::list_sessions(1),
            RPCRequest::revoke_session(2, &id),
            RPCRequest::contact_sync(3, 5),
            RPCRequest::transfer_channel_ownership(4, &id),
            RPCRequest::ban_channel_members(5, &[id, id]),
            RPCRequest::get_channel_info(6).with_cookie(vec![1, 2]),
        ] {
            let parsed = RPCRequest::parse(&request.serialize().unwrap()).unwrap();
            assert_eq!(parsed.method, request.method);
            assert_eq!(parsed.params, request.params);
            assert_eq!(parsed.cookie, request.cookie);
        }
        assert!(RPCRequest::new(1, Method::SessionRevoke, None).serialize().is_err());
        assert!(RPCRequest::new(-1, Method::SessionList, None).serialize().is_err());
    }
}
