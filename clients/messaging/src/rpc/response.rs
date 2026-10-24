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
use serde_cbor::{self, Value};
use super::error::RPCError;

use boson_api::{
    Error,
    Result
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

    pub(crate) fn result<T>(&mut self) -> Result<T>
    where
        T: serde::de::DeserializeOwned
    {
        if let Some(v) = self.result.take() {
            return serde_cbor::value::from_value(v).map_err(|e|
                Error::Protocol(format!("Internal error {e}: bad RPC response"))
            )
        }
        if let Some(e) = self.error.take() {
            // TODO:
            return Err(Error::Protocol(format!("Server error: {e}")))

        }
        Err(Error::Protocol("Incomplete RPC response, missing both result and error fields".to_string()))
    }

    pub(crate) fn error(&self) -> Option<&RPCError> {
        self.error.as_ref()
    }

    // TODO:
}
