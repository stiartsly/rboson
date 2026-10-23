//! MQTT control frames retain the Java protocol's compact CBOR field names.
//! Each pending call owns a oneshot promise; dropping its future unregisters it.
use std::{cell::RefCell, collections::HashMap};
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use tokio::sync::oneshot;
use boson_api::{Id, errors::{Result, StateError}};
use crate::errors::{AuthenticationError, EncodingError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Method {
    #[serde(rename = "sl")]
    SessionList,
    #[serde(rename = "sr")]
    SessionRevoke,
    #[serde(rename = "cm")]
    ContactMutate,
    #[serde(rename = "cs")]
    ContactSync,
    #[serde(rename = "cc")]
    ChannelCreate,
    #[serde(rename = "cd")]
    ChannelDelete,
    #[serde(rename = "cot")]
    ChannelOwnershipTransfer,
    #[serde(rename = "csr")]
    ChannelSessionKeyRotate,
    #[serde(rename = "ciu")]
    ChannelInfoUpdate,
    #[serde(rename = "cru")]
    ChannelMembersRoleUpdate,
    #[serde(rename = "cmb")]
    ChannelMembersBan,
    #[serde(rename = "cmu")]
    ChannelMembersUnban,
    #[serde(rename = "cmr")]
    ChannelMembersRemove,
    #[serde(rename = "cj")]
    ChannelJoin,
    #[serde(rename = "cl")]
    ChannelLeave,
    #[serde(rename = "ci")]
    ChannelInfo,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Request {
    pub(crate) id: i64,
    #[serde(rename = "m")]
    pub(crate) method: Method,
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<Value>,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub(crate) cookie: Option<Value>,
}

impl Request {
    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding RPC request failed: {error}")))?)
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Response {
    pub(crate) id: i64,
    #[serde(rename = "m")]
    pub(crate) method: Method,
    #[serde(rename = "r", default, skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<RpcError>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct RpcError {
    #[serde(rename = "c")]
    pub(crate) code: i32,
    #[serde(rename = "m")]
    pub(crate) message: String,
}

impl Response {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let response: Self = serde_cbor::from_slice(bytes)
            .map_err(|error| EncodingError::new(format!("Malformed RPC response: {error}")))?;
        if response.id < 0 {
            return Err(EncodingError::new("RPC response ID is negative"));
        }
        Ok(response)
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding RPC response failed: {error}")))?)
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

    fn into_result(self) -> std::result::Result<Value, String> {
        self.validate().map_err(|error| error.to_string())?;
        if let Some(error) = self.error {
            Err(format!("RPC '{:?}' failed ({}): {}", self.method, error.code, error.message))
        } else {
            Ok(self.result.unwrap_or(Value::Null))
        }
    }
}

struct Promise {
    method: Method,
    recipient: Id,
    complete: oneshot::Sender<std::result::Result<Value, String>>,
}

#[derive(Default)]
pub(crate) struct Calls {
    pending: RefCell<HashMap<i64, Promise>>,
}

pub(crate) struct Call<'a> {
    calls: &'a Calls,
    id: i64,
    response: oneshot::Receiver<std::result::Result<Value, String>>,
}

impl Drop for Call<'_> {
    fn drop(&mut self) {
        self.calls.pending.borrow_mut().remove(&self.id);
    }
}

impl Call<'_> {
    pub(crate) async fn response(&mut self) -> Result<Value> {
        (&mut self.response).await
            .map_err(|_| StateError::new("RPC response channel closed"))?
            .map_err(|error| StateError::new(error).into())
    }
}

impl Calls {
    pub(crate) fn register(&self, id: i64, method: Method, recipient: Id) -> Result<Call<'_>> {
        let mut pending = self.pending.borrow_mut();
        if id < 0 || pending.contains_key(&id) {
            return Err(StateError::new("Invalid or duplicate pending RPC ID"));
        }
        let (complete, response) = oneshot::channel();
        pending.insert(id, Promise { method, recipient, complete });
        Ok(Call { calls: self, id, response })
    }

    pub(crate) fn respond(&self, response: Response, sender: &Id) -> Result<()> {
        if self.pending.borrow().get(&response.id).is_some_and(|call| &call.recipient != sender) {
            return Err(AuthenticationError::new("RPC response sender does not match request recipient"));
        }
        let Some(call) = self.pending.borrow_mut().remove(&response.id) else {
            log::debug!("Ignoring RPC response {} without a pending call", response.id);
            return Ok(());
        };
        let result = if call.method != response.method {
            Err(format!("RPC response method '{:?}' does not match '{:?}'", response.method, call.method))
        } else {
            response.into_result()
        };
        if let Err(error) = &result {
            log::warn!("{error}");
        }
        let _ = call.complete.send(result);
        Ok(())
    }

    pub(crate) fn fail_all(&self, reason: &str) {
        for (_, call) in self.pending.take() {
            let _ = call.complete.send(Err(reason.into()));
        }
    }
}

        #[cfg(test)]
        mod tests {
            use super::*;
            use std::time::Duration;

            fn response(id: i64, method: Method) -> Response {
                Response { id, method, result: None, error: None }
            }

            #[test]
            fn rpc_wire_roundtrip_and_validation() {
                let request = Request {
                    id: 17, method: Method::ContactSync, params: Some(Value::Integer(3)),
                    cookie: Some(Value::Bytes(vec![1, 2, 3])),
                };
                let bytes = request.serialize().unwrap();
                assert_eq!(Request::parse(&bytes).unwrap(), request);
                let Value::Map(wire) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else { panic!("RPC must be a map") };
                assert_eq!(wire.get(&Value::Text("m".into())), Some(&Value::Text("cs".into())));
                assert_eq!(wire.get(&Value::Text("p".into())), Some(&Value::Integer(3)));
                assert_eq!(wire.get(&Value::Text("c".into())), Some(&Value::Bytes(vec![1, 2, 3])));
                let mut invalid = Request { params: Some(Value::Text("wrong".into())), ..request };
                assert!(invalid.serialize().is_err());
                assert!(Request::parse(&serde_cbor::to_vec(&invalid).unwrap()).is_err());
                invalid.params = Some(Value::Integer(-1));
                assert!(invalid.serialize().is_err());
                assert!(Request::parse(&[0xff]).is_err());
                assert!(Request::parse(&[0xa2, 0x62, b'i', b'd', 0x01, 0x61, b'm', 0x61, b'?']).is_err());

                let mut success = response(17, Method::ChannelLeave);
                let bytes = success.serialize().unwrap();
                assert_eq!(Response::parse(&bytes).unwrap(), success);
                assert_eq!(Response::parse(&bytes).unwrap().into_result().unwrap(), Value::Null);
                success.error = Some(RpcError { code: 403, message: "denied".into() });
                let parsed = Response::parse(&success.serialize().unwrap()).unwrap();
                assert_eq!(parsed, success);
                assert!(parsed.into_result().unwrap_err().contains("denied"));
                success.result = Some(Value::Integer(1));
                assert!(success.serialize().is_err());
                assert!(Response::parse(&[0xff]).is_err());
            }

            #[tokio::test]
            async fn rpc_responses_correlate_and_authenticate() {
                let calls = Calls::default();
                let sender = Id::random();
                let mut first = calls.register(1, Method::ChannelLeave, sender).unwrap();
                let mut second = calls.register(2, Method::ChannelDelete, sender).unwrap();
                assert!(calls.register(1, Method::ChannelLeave, sender).is_err());
                assert!(calls.respond(response(1, Method::ChannelLeave), &Id::random()).is_err());
                assert_eq!(calls.pending.borrow().len(), 2);
                calls.respond(response(2, Method::ChannelDelete), &sender).unwrap();
                calls.respond(response(1, Method::ChannelLeave), &sender).unwrap();
                assert_eq!(second.response().await.unwrap(), Value::Null);
                assert_eq!(first.response().await.unwrap(), Value::Null);
                assert!(calls.pending.borrow().is_empty());
                assert!(calls.respond(response(1, Method::ChannelLeave), &sender).is_ok());
            }

            #[tokio::test]
            async fn rpc_method_mismatch_and_invalid_result_complete_with_errors() {
                let calls = Calls::default();
                let sender = Id::random();
                let mut call = calls.register(1, Method::ChannelLeave, sender).unwrap();
                calls.respond(response(1, Method::ChannelDelete), &sender).unwrap();
                assert!(call.response().await.unwrap_err().to_string().contains("does not match"));
                let mut call = calls.register(2, Method::ChannelLeave, sender).unwrap();
                let mut invalid = response(2, Method::ChannelLeave);
                invalid.result = Some(Value::Integer(1));
                invalid.error = Some(RpcError { code: 1, message: "error".into() });
                calls.respond(invalid, &sender).unwrap();
                assert!(call.response().await.is_err());
                let mut call = calls.register(3, Method::SessionList, sender).unwrap();
                calls.respond(response(3, Method::SessionList), &sender).unwrap();
                assert!(call.response().await.is_err());
                assert!(calls.pending.borrow().is_empty());
            }

            #[tokio::test]
            async fn rpc_cancellation_timeout_and_shutdown_remove_promises() {
                let calls = Calls::default();
                let sender = Id::random();
                drop(calls.register(1, Method::ChannelLeave, sender).unwrap());
                assert!(calls.pending.borrow().is_empty());
                {
                    let mut call = calls.register(2, Method::ChannelLeave, sender).unwrap();
                    assert!(tokio::time::timeout(Duration::from_millis(1), call.response()).await.is_err());
                }
                assert!(calls.pending.borrow().is_empty());
                let mut first = calls.register(3, Method::ChannelLeave, sender).unwrap();
                let mut second = calls.register(4, Method::ChannelLeave, sender).unwrap();
                calls.fail_all("disconnected");
                assert!(first.response().await.unwrap_err().to_string().contains("disconnected"));
                assert!(second.response().await.unwrap_err().to_string().contains("disconnected"));
                assert!(calls.pending.borrow().is_empty());
            }
        }
