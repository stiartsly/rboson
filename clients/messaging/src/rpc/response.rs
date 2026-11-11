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
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_cbor::Value;

use boson_api::{Id, Result};
use crate::errors::EncodingError;
use crate::internal::{
    ChannelInfo,
    ContactSync,
};
use crate::session_info::SessionInfo;
use super::{
    error::RPCError,
    method::Method,
};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Params {
    SessionList(Vec<SessionInfo>),
    SessionRevoke(()),
    ContactMutate(i32),
    ContactSync(ContactSync),
    ChannelCreate(ChannelInfo),
    ChannelDelete(()),
    ChannelOwnershipTransfer(()),
    ChannelSessionKeyRotate(()),
    ChannelInfoUpdate(()),
    ChannelMembersRoleUpdate(Vec<Id>),
    ChannelMembersBan(Vec<Id>),
    ChannelMembersUnban(Vec<Id>),
    ChannelMembersRemove(Vec<Id>),
    ChannelJoin(ChannelInfo),
    ChannelLeave(()),
    ChannelInfo(ChannelInfo),
}

impl Params {
    fn empty(method: Method) -> Option<Self> {
        match method {
            Method::SessionRevoke => Some(Self::SessionRevoke(())),
            Method::ChannelDelete => Some(Self::ChannelDelete(())),
            Method::ChannelOwnershipTransfer => Some(Self::ChannelOwnershipTransfer(())),
            Method::ChannelSessionKeyRotate => Some(Self::ChannelSessionKeyRotate(())),
            Method::ChannelInfoUpdate => Some(Self::ChannelInfoUpdate(())),
            Method::ChannelLeave => Some(Self::ChannelLeave(())),
            _ => None,
        }
    }

    fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
        serde_cbor::value::from_value(value).map_err(|error|
            EncodingError::new(format!("Malformed response parameters: {error}")).into()
        )
    }

    fn from_result(method: Method, value: Value) -> Result<Self> {
        let malformed = || EncodingError::new(format!("Malformed result for {method:?}"));

        match (method, value) {
            (Method::SessionList, value @ Value::Array(_)) => {
                Ok(Self::SessionList(Self::decode(value)?))
            }
            (Method::SessionRevoke, Value::Null) => Ok(Self::SessionRevoke(())),
            (Method::ContactMutate, Value::Integer(revision)) => {
                let revision = i32::try_from(revision)
                    .ok()
                    .filter(|revision| *revision >= 0)
                    .ok_or_else(malformed)?;
                Ok(Self::ContactMutate(revision))
            }
            (Method::ContactSync, value @ Value::Map(_)) => {
                Ok(Self::ContactSync(Self::decode(value)?))
            }
            (Method::ChannelCreate, value @ Value::Map(_)) => {
                Ok(Self::ChannelCreate(Self::decode(value)?))
            }
            (Method::ChannelDelete, Value::Null) => Ok(Self::ChannelDelete(())),
            (Method::ChannelOwnershipTransfer, Value::Null) => {
                Ok(Self::ChannelOwnershipTransfer(()))
            }
            (Method::ChannelSessionKeyRotate, Value::Null) => {
                Ok(Self::ChannelSessionKeyRotate(()))
            }
            (Method::ChannelInfoUpdate, Value::Null) => Ok(Self::ChannelInfoUpdate(())),
            (Method::ChannelMembersRoleUpdate, value) => {
                Ok(Self::ChannelMembersRoleUpdate(Self::decode(value)?))
            }
            (Method::ChannelMembersBan, value) => {
                Ok(Self::ChannelMembersBan(Self::decode(value)?))
            }
            (Method::ChannelMembersUnban, value) => {
                Ok(Self::ChannelMembersUnban(Self::decode(value)?))
            }
            (Method::ChannelMembersRemove, value) => {
                Ok(Self::ChannelMembersRemove(Self::decode(value)?))
            }
            (Method::ChannelJoin, value @ Value::Map(_)) => {
                Ok(Self::ChannelJoin(Self::decode(value)?))
            }
            (Method::ChannelLeave, Value::Null) => Ok(Self::ChannelLeave(())),
            (Method::ChannelInfo, value @ Value::Map(_)) => {
                Ok(Self::ChannelInfo(Self::decode(value)?))
            }
            _ => Err(malformed().into()),
        }
    }

    /// The touched members of a member-management result.
    pub(crate) fn into_members(self) -> Result<Vec<Id>> {
        match self {
            Self::ChannelMembersRoleUpdate(ids)
            | Self::ChannelMembersBan(ids)
            | Self::ChannelMembersUnban(ids)
            | Self::ChannelMembersRemove(ids) => Ok(ids),
            _ => Err(EncodingError::new("RPC result is not a member list").into()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawResponse")]
pub(crate) struct RpcResponse {
    id: i64,
    method: Method,
    result: Option<Params>,
    error: Option<RPCError>,
}

impl RpcResponse {
    pub(crate) fn id(&self) -> i64 {
        self.id
    }

    pub(crate) fn method(&self) -> Method {
        self.method
    }

    pub(crate) fn result(self) -> Result<Params> {
        if let Some(error) = self.error {
            return Err(error.into());
        }
        match self.result {
            Some(params) => Ok(params),
            None => Params::empty(self.method)
                .ok_or_else(|| EncodingError::new(format!("Missing result for {:?}", self.method)).into()),
        }
    }
}

#[derive(Deserialize)]
struct RawResponse {
    #[serde(rename = "id")]
    id: i64,
    #[serde(rename = "m")]
    method: Method,
    #[serde(rename = "r", default)]
    result: Option<Value>,
    #[serde(rename = "e", default)]
    error: Option<RPCError>,
}

impl TryFrom<RawResponse> for RpcResponse {
    type Error = Box<dyn std::error::Error>;

    fn try_from(raw: RawResponse) -> StdResult<Self, Self::Error> {
        if raw.id < 0 {
            return Err(EncodingError::new("Invalid RPC response id").into());
        }
        let result = match raw.result {
            Some(value) => Some(Params::from_result(raw.method, value)?),
            None => None,
        };
        Ok(Self {
            id: raw.id,
            method: raw.method,
            result,
            error: raw.error,
        })
    }
}
