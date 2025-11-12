use serde::{Deserialize, Serialize};
use serde_cbor::{self, Value};
use super::error::RPCError;

use crate::{
    Error,
    error::Result
};

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RPCResponse
{
    #[serde(rename = "i")]
    id: u32,

    #[serde(rename = "r", skip_serializing_if = "Option::is_none")]
    result: Option<Value>,

    #[serde(rename = "e", skip_serializing_if = "Option::is_none")]
    error: Option<RPCError>
}

#[allow(unused)]
impl RPCResponse
{
    pub(crate) fn new<T>(id: u32, result: T) -> Self where T: Serialize {
        Self {
            id,
            result: serde_cbor::value::to_value(result).ok(),
            error: None,
        }
    }

    pub(crate) fn from(body: &[u8]) -> Result<Self> {
        serde_cbor::from_slice::<RPCResponse>(body).map_err(|e|
            Error::Protocol(format!("Failed to parse RPC response: {}", e))
        )
    }

    pub(crate) fn with_error(id: u32, error: RPCError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
        }
    }

    pub(crate) fn with_error_details(id: u32, code: i32, message: &str, data: Option<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(RPCError::new(code, message, data)),
        }
    }

    pub(crate) fn id(&self) -> &u32 {
        &self.id
    }

    pub(crate) fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    pub(crate) fn failed(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn result<T>(&self) -> Option<T> where T: serde::de::DeserializeOwned {
        if let Some(ref v) = self.result {
            serde_cbor::value::from_value(v.clone()).ok()
        } else {
            None
        }
    }

    pub(crate) fn error(&self) -> Option<&RPCError> {
        self.error.as_ref()
    }

    // TODO:
}
