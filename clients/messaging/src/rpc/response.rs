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

use serde::{Deserialize, Serialize, Serializer};
use serde_cbor::Value;

use boson_api::{Id, Result};
use crate::errors::EncodingError;
use super::{
    error::RPCError,
    method::Method,
    params::id_list
};

#[derive(Debug, PartialEq)]
pub(crate) enum Params {
    /// Operations that return nothing.
    Empty,
    /// The session list returned by `SessionList`.
    Sessions(Value),
    /// The revision reached by a contact mutation.
    Revision(i32),
    /// The document returned by `ContactSync`.
    ContactSync(Value),
    /// The channel document returned by `ChannelCreate` or `ChannelJoin`.
    Channel(Value),
    /// The channel document returned by `ChannelInfo`.
    ChannelInfo(Value),
    /// The members a member-management operation actually touched.
    Members(Vec<Id>),
}

impl Params {
    /// Whether a successful reply for `method` carries no result body.
    fn accepts_empty(method: Method) -> bool {
        matches!(
            method,
            Method::SessionRevoke
                | Method::ChannelDelete
                | Method::ChannelLeave
                | Method::ChannelOwnershipTransfer
                | Method::ChannelSessionKeyRotate
                | Method::ChannelInfoUpdate
        )
    }

    /// Interprets the `r` field of a reply according to `method`.
    pub(crate) fn decode(method: Method, value: Value) -> Result<Self> {
        let malformed = || EncodingError::new(format!("Malformed result for {method:?}"));
        match (method, value) {
            (
                Method::SessionRevoke
                | Method::ChannelDelete
                | Method::ChannelLeave
                | Method::ChannelOwnershipTransfer
                | Method::ChannelSessionKeyRotate
                | Method::ChannelInfoUpdate,
                Value::Null,
            ) => Ok(Self::Empty),
            (Method::ContactMutate, Value::Integer(revision)) => {
                let revision = i32::try_from(revision)
                    .ok()
                    .filter(|revision| *revision >= 0)
                    .ok_or_else(malformed)?;
                Ok(Self::Revision(revision))
            }
            (
                Method::ChannelMembersRoleUpdate
                | Method::ChannelMembersBan
                | Method::ChannelMembersUnban
                | Method::ChannelMembersRemove,
                value,
            ) => {
                let members =
                    serde_cbor::value::from_value::<Vec<Id>>(value).map_err(|_| malformed())?;
                Ok(Self::Members(members))
            }
            (Method::SessionList, value @ Value::Array(_)) => Ok(Self::Sessions(value)),
            (Method::ContactSync, value @ Value::Map(_)) => Ok(Self::ContactSync(value)),
            (Method::ChannelCreate | Method::ChannelJoin, value @ Value::Map(_)) => {
                Ok(Self::Channel(value))
            }
            (Method::ChannelInfo, value @ Value::Map(_)) => Ok(Self::ChannelInfo(value)),
            _ => Err(malformed()),
        }
    }

    /// The plain CBOR value a pending call is completed with.
    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Empty => Value::Null,
            Self::Sessions(document)
            | Self::ContactSync(document)
            | Self::Channel(document)
            | Self::ChannelInfo(document) => document,
            Self::Revision(revision) => Value::Integer(revision.into()),
            Self::Members(ids) => Value::Array(
                ids.iter()
                    .map(|id| Value::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
        }
    }

    /// `None` is the wire form of an empty reply; `Some` carries a body.
    fn into_option(self) -> Option<Self> {
        match self {
            Self::Empty => None,
            result => Some(result),
        }
    }
}

impl Serialize for Params {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Empty => serializer.serialize_unit(),
            Self::Revision(revision) => revision.serialize(serializer),
            Self::Members(ids) => id_list::serialize(ids, serializer),
            Self::Sessions(document)
            | Self::ContactSync(document)
            | Self::Channel(document)
            | Self::ChannelInfo(document) => document.serialize(serializer),
        }
    }
}

/// Wire shape of a reply, before its result is interpreted by method.
#[derive(Deserialize, Serialize)]
struct Envelope {
    id: i64,
    #[serde(rename = "m")]
    method: Method,
    #[serde(rename = "r", default)]
    result: Option<Value>,
    #[serde(rename = "e", default)]
    error: Option<RPCError>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RPCResponse {
    pub(crate) id: i64,
    #[serde(rename = "m")]
    pub(crate) method: Method,
    #[serde(rename = "r", skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Params>,
    #[serde(rename = "e", skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<RPCError>,
}

#[allow(dead_code)]
impl RPCResponse {
    fn new(id: i64, method: Method, result: Option<Params>) -> Self {
        Self {
            id,
            method,
            result,
            error: None,
        }
    }

    pub(crate) fn success(id: i64, method: Method, result: Params) -> Self {
        Self::new(id, method, result.into_option())
    }

    pub(crate) fn empty(id: i64, method: Method) -> Self {
        debug_assert!(Params::accepts_empty(method));
        Self::new(id, method, None)
    }

    pub(crate) fn error(id: i64, method: Method, error: RPCError) -> Self {
        Self {
            id,
            method,
            result: None,
            error: Some(error),
        }
    }

    pub(crate) fn list_sessions(id: i64, sessions: Value) -> Self {
        Self::success(id, Method::SessionList, Params::Sessions(sessions))
    }

    pub(crate) fn revoke_session(id: i64) -> Self {
        Self::empty(id, Method::SessionRevoke)
    }

    pub(crate) fn contact_mutate(id: i64, revision: i32) -> Self {
        Self::success(
            id,
            Method::ContactMutate,
            Params::Revision(revision),
        )
    }

    pub(crate) fn contact_sync(id: i64, document: Value) -> Self {
        Self::success(
            id,
            Method::ContactSync,
            Params::ContactSync(document),
        )
    }

    pub(crate) fn create_channel(id: i64, channel: Value) -> Self {
        Self::success(id, Method::ChannelCreate, Params::Channel(channel))
    }

    pub(crate) fn join_channel(id: i64, channel: Value) -> Self {
        Self::success(id, Method::ChannelJoin, Params::Channel(channel))
    }

    pub(crate) fn delete_channel(id: i64) -> Self {
        Self::empty(id, Method::ChannelDelete)
    }

    pub(crate) fn leave_channel(id: i64) -> Self {
        Self::empty(id, Method::ChannelLeave)
    }

    pub(crate) fn get_channel_info(id: i64, info: Value) -> Self {
        Self::success(id, Method::ChannelInfo, Params::ChannelInfo(info))
    }

    pub(crate) fn transfer_channel_ownership(id: i64) -> Self {
        Self::empty(id, Method::ChannelOwnershipTransfer)
    }

    pub(crate) fn rotate_channel_session_key(id: i64) -> Self {
        Self::empty(id, Method::ChannelSessionKeyRotate)
    }

    pub(crate) fn update_channel_info(id: i64) -> Self {
        Self::empty(id, Method::ChannelInfoUpdate)
    }

    pub(crate) fn update_channel_members_role(id: i64, members: Vec<Id>) -> Self {
        Self::success(
            id,
            Method::ChannelMembersRoleUpdate,
            Params::Members(members),
        )
    }

    pub(crate) fn ban_channel_members(id: i64, members: Vec<Id>) -> Self {
        Self::success(
            id,
            Method::ChannelMembersBan,
            Params::Members(members),
        )
    }

    pub(crate) fn unban_channel_members(id: i64, members: Vec<Id>) -> Self {
        Self::success(
            id,
            Method::ChannelMembersUnban,
            Params::Members(members),
        )
    }

    pub(crate) fn remove_channel_members(id: i64, members: Vec<Id>) -> Self {
        Self::success(
            id,
            Method::ChannelMembersRemove,
            Params::Members(members),
        )
    }

    /// Builds a response from the already extracted result of a call.
    pub(crate) fn from_value(method: Method, value: Value) -> Result<Self> {
        let result = Params::decode(method, value)?;
        Ok(Self::new(0, method, result.into_option()))
    }

    pub(crate) fn error_with(
        id: i64,
        method: Method,
        code: i32,
        message: &str,
        data: Option<String>,
    ) -> Self {
        Self::error(id, method, RPCError::new(code, message, data))
    }

    pub(crate) fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    pub(crate) fn failed(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let envelope: Envelope = serde_cbor::from_slice(bytes)
            .map_err(|error| EncodingError::new(format!("Malformed RPC response: {error}")))?;
        if envelope.id < 0 {
            return Err(EncodingError::new("Invalid RPC response ID").into());
        }
        if let Some(error) = envelope.error {
            return Ok(Self::error(envelope.id, envelope.method, error));
        }
        let result = match envelope.result {
            Some(value) => Params::decode(envelope.method, value)?.into_option(),
            None if Params::accepts_empty(envelope.method) => None,
            None => {
                return Err(
                    EncodingError::new(format!("Missing result for {:?}", envelope.method)).into(),
                )
            }
        };
        Ok(Self::new(envelope.id, envelope.method, result))
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            EncodingError::new(format!("Encoding RPC response failed: {error}")).into()
        })
    }

    /// Yields the result, or the service's `RPCError` as a typed error.
    pub(crate) fn into_result(self) -> Result<Params> {
        match (self.error, self.result) {
            (Some(error), _) => Err(error.into()),
            (None, Some(result)) => Ok(result),
            (None, None) => Ok(Params::Empty),
        }
    }

    /// The touched members of a member-management response.
    pub(crate) fn into_members(self) -> Result<Vec<Id>> {
        match self.into_result()? {
            Params::Members(ids) => Ok(ids),
            _ => Err(EncodingError::new("RPC result is not a member list").into()),
        }
    }
}
