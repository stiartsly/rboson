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
use super::{error::RPCError, method::Method};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RPCResponse {
    pub(crate) id: i64,
    #[serde(rename = "m")]
    pub(crate) method: Method,
    #[serde(rename = "r", default, skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<RPCError>,
}

#[allow(dead_code)]
impl RPCResponse {
    pub(crate) fn new(id: i64, method: Method, result: Option<Value>) -> Self {
        Self { id, method, result, error: None }
    }

    pub(crate) fn list_sessions(id: i64, sessions: Value) -> Self {
        Self::new(id, Method::SessionList, Some(sessions))
    }

    pub(crate) fn revoke_session(id: i64) -> Self {
        Self::new(id, Method::SessionRevoke, None)
    }

    pub(crate) fn contact_mutate(id: i64, revision: i32) -> Self {
        Self::new(id, Method::ContactMutate, Some(Value::Integer(revision.into())))
    }

    pub(crate) fn create_channel(id: i64, channel_info: Value) -> Self {
        Self::new(id, Method::ChannelCreate, Some(channel_info))
    }

    pub(crate) fn delete_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelDelete, None)
    }

    pub(crate) fn join_channel(id: i64, channel_info: Value) -> Self {
        Self::new(id, Method::ChannelJoin, Some(channel_info))
    }

    pub(crate) fn leave_channel(id: i64) -> Self {
        Self::new(id, Method::ChannelLeave, None)
    }

    pub(crate) fn transfer_channel_ownership(id: i64) -> Self {
        Self::new(id, Method::ChannelOwnershipTransfer, None)
    }

    pub(crate) fn rotate_channel_session_key(id: i64) -> Self {
        Self::new(id, Method::ChannelSessionKeyRotate, None)
    }

    pub(crate) fn update_channel_info(id: i64) -> Self {
        Self::new(id, Method::ChannelInfoUpdate, None)
    }

    pub(crate) fn update_channel_members_role(id: i64, member_ids: &[Id]) -> Self {
        Self::new(id, Method::ChannelMembersRoleUpdate, Some(id_list(member_ids)))
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

    pub(crate) fn get_channel_info(id: i64, channel_info: Value) -> Self {
        Self::new(id, Method::ChannelInfo, Some(channel_info))
    }

    pub(crate) fn error(id: i64, method: Method, error: RPCError) -> Self {
        Self { id, method, result: None, error: Some(error) }
    }

    pub(crate) fn error_with(id: i64, method: Method, code: i32, message: &str, data: Option<String>) -> Self {
        Self::error(id, method, RPCError::new(code, message, data))
    }

    pub(crate) fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    pub(crate) fn failed(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let response: Self = serde_cbor::from_slice(bytes)
            .map_err(|error| EncodingError::new(format!("Malformed RPC response: {error}")))?;
        response.validate()?;
        Ok(response)
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding RPC response failed: {error}")).into())
    }

    fn validate(&self) -> Result<()> {
        if self.id < 0 || (self.result.is_some() && self.error.is_some()) {
            return Err(EncodingError::new("Invalid RPC response ID or result/error combination"));
        }
        if self.error.is_some() {
            return Ok(());
        }
        let valid = match (self.method, self.result.as_ref()) {
            (Method::SessionRevoke | Method::ChannelDelete | Method::ChannelOwnershipTransfer
                | Method::ChannelSessionKeyRotate | Method::ChannelInfoUpdate | Method::ChannelLeave,
                None | Some(Value::Null)) => true,
            (Method::SessionList | Method::ChannelMembersRoleUpdate | Method::ChannelMembersBan
                | Method::ChannelMembersUnban | Method::ChannelMembersRemove, Some(Value::Array(_))) => true,
            (Method::ContactMutate, Some(Value::Integer(revision))) => i32::try_from(*revision).is_ok() && *revision >= 0,
            (Method::ContactSync | Method::ChannelCreate | Method::ChannelJoin | Method::ChannelInfo,
                Some(Value::Map(_))) => true,
            _ => false,
        };
        if !valid {
            return Err(EncodingError::new("RPC result does not match the method"));
        }
        Ok(())
    }

    /// Yields the result, or the service's `RPCError` as a typed error.
    pub(crate) fn into_result(self) -> Result<Value> {
        self.validate()?;
        if let Some(error) = self.error {
            return Err(error.into());
        }
        Ok(self.result.unwrap_or(Value::Null))
    }
}

fn id_list(ids: &[Id]) -> Value {
    Value::Array(ids.iter().map(|id| Value::Bytes(id.as_bytes().to_vec())).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::error::code;

    #[test]
    fn response_round_trips_and_maps_errors() {
        let ok = RPCResponse::contact_mutate(7, 3);
        let parsed = RPCResponse::parse(&ok.serialize().unwrap()).unwrap();
        assert!(parsed.succeeded());
        assert_eq!(parsed.into_result().unwrap(), Value::Integer(3));

        let failed = RPCResponse::error(8, Method::ChannelJoin, RPCError::forbidden_banned_channel_member());
        let parsed = RPCResponse::parse(&failed.serialize().unwrap()).unwrap();
        assert!(parsed.failed());
        let error = parsed.into_result().unwrap_err();
        let error = error.downcast_ref::<RPCError>().unwrap();
        assert_eq!(error.code(), code::FORBIDDEN_BANNED_CHANNEL_MEMBER);
    }

    #[test]
    fn invalid_results_are_rejected() {
        assert!(RPCResponse::new(1, Method::ChannelInfo, None).serialize().is_err());
        assert!(RPCResponse::new(-1, Method::ChannelLeave, None).serialize().is_err());
    }
}
