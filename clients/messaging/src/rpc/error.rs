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

use std::fmt;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

#[allow(dead_code)]
pub(crate) static SUPERNODE_ERR: Lazy<RPCError>  = Lazy::new(|| RPCError::new(-1, "Super node internal error", None));
#[allow(dead_code)]
pub(crate) static INVALID_PARAMS: Lazy<RPCError> = Lazy::new(|| RPCError::new(-2, "Invalid parameters", None));
#[allow(dead_code)]
pub(crate) static INVALID_METHOD: Lazy<RPCError> = Lazy::new(|| RPCError::new(-3, "Invalid method", None));
#[allow(dead_code)]
pub(crate) static FORBIDDEN: Lazy<RPCError>      = Lazy::new(|| RPCError::new(-4, "Forbidden", None));
#[allow(dead_code)]
pub(crate) static TIMEOUT: Lazy<RPCError>        = Lazy::new(|| RPCError::new(-5, "Timeout", None));
#[allow(dead_code)]
pub(crate) static NOT_UP_TO_DATE: Lazy<RPCError> = Lazy::new(|| RPCError::new(-6, "Not up to date", None));
#[allow(dead_code)]
pub(crate) static ALREADY_EXISTS: Lazy<RPCError> = Lazy::new(|| RPCError::new(-7, "Already exists", None));

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RPCError {
    #[serde(rename = "c")]
    code: i32,

    #[serde(rename = "m")]
    message: String,

    #[serde(rename = "d", skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

#[allow(unused)]
impl RPCError {
    pub fn new(code: i32, message: &str, data: Option<String>) -> Self {
        Self {
            code,
            message: message.to_string(),
            data,
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }
}

impl fmt::Display for RPCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RPCError [code={}, message={}]", self.code, self.message)
    }
}
