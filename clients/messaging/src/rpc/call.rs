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

use std::{cell::RefCell, collections::HashMap, future::Future, pin::Pin, task::{Context, Poll}};

use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use boson_api::{Id, Promise, PromiseFuture, Result};
use boson_api::errors::StateError;
use crate::errors::{AuthenticationError, EncodingError};
use super::{error::RPCError, method::Method};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RPCRequest {
    pub(crate) id: i64,
    #[serde(rename = "m")]
    pub(crate) method: Method,
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<Value>,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub(crate) cookie: Option<Value>,
}

impl RPCRequest {
    pub(crate) fn new(id: i64, method: Method, params: Option<Value>) -> Self {
        Self { id, method, params, cookie: None }
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

impl RPCResponse {
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

    fn into_result(self) -> Result<Value> {
        self.validate()?;
        if let Some(error) = self.error {
            return Err(StateError::new(format!(
                "RPC '{:?}' failed ({}): {}",
                self.method,
                error.code(),
                error.message()
            )).into());
        }
        Ok(self.result.unwrap_or(Value::Null))
    }
}

struct PendingCall {
    method: Method,
    recipient: Id,
    promise: Promise<Value>,
}

#[derive(Default)]
pub(crate) struct Calls {
    pending: RefCell<HashMap<i64, PendingCall>>,
}

pub(crate) struct CallFuture<'a> {
    calls: &'a Calls,
    id: i64,
    future: PromiseFuture<Value>,
}

impl Future for CallFuture<'_> {
    type Output = Result<Value>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().future).poll(context)
    }
}

impl Drop for CallFuture<'_> {
    fn drop(&mut self) {
        self.calls.pending.borrow_mut().remove(&self.id);
    }
}

impl Calls {
    pub(crate) fn register(&self, id: i64, method: Method, recipient: Id) -> Result<CallFuture<'_>> {
        if id < 0 || self.pending.borrow().contains_key(&id) {
            return Err(StateError::new("Invalid or duplicate pending RPC ID"));
        }
        let (promise, future) = Promise::pair();
        self.pending.borrow_mut().insert(id, PendingCall { method, recipient, promise });
        Ok(CallFuture { calls: self, id, future })
    }

    pub(crate) fn respond(&self, response: RPCResponse, sender: &Id) -> Result<()> {
        if self.pending.borrow().get(&response.id).is_some_and(|call| &call.recipient != sender) {
            return Err(AuthenticationError::new("RPC response sender does not match request recipient"));
        }
        let Some(call) = self.pending.borrow_mut().remove(&response.id) else {
            log::debug!("Ignoring RPC response {} without a pending call", response.id);
            return Ok(());
        };
        let result = if call.method != response.method {
            Err(EncodingError::new(format!(
                "RPC response method '{:?}' does not match '{:?}'",
                response.method,
                call.method
            )).into())
        } else {
            response.into_result()
        };
        if let Err(error) = &result {
            log::warn!("{error}");
        }
        call.promise.complete(result);
        Ok(())
    }

    pub(crate) fn fail_all(&self, reason: &str) {
        for (_, call) in self.pending.take() {
            call.promise.complete(Err(StateError::new(reason.to_string()).into()));
        }
    }
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

        let request = RPCRequest::new(17, Method::ChannelDelete, None);
        let parsed = RPCRequest::parse(&request.serialize().unwrap()).unwrap();

        assert_eq!(parsed.id, request.id);
        assert_eq!(parsed.method, request.method);
        assert_eq!(parsed.params, request.params);
    }

    #[test]
    fn response_completes_the_matching_pending_call() {
        futures::executor::block_on(async {
            let calls = Calls::default();
            let recipient = Id::default();
            let future = calls.register(17, Method::ChannelDelete, recipient).unwrap();
            let response = RPCResponse {
                id: 17,
                method: Method::ChannelDelete,
                result: None,
                error: None,
            };
            let response = RPCResponse::parse(&response.serialize().unwrap()).unwrap();

            assert!(calls.respond(response, &Id::from_bytes([1; Id::BYTES])).is_err());
            calls.respond(
                RPCResponse {
                    id: 17,
                    method: Method::ChannelDelete,
                    result: None,
                    error: None,
                },
                &recipient,
            ).unwrap();

            assert_eq!(future.await.unwrap(), Value::Null);
        });
    }
}
