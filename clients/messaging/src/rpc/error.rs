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

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

/// Wire-level RPC error codes shared with the service and the Java client.
#[allow(dead_code)]
pub(crate) mod code {
    // Common error codes
    pub(crate) const RPC_INTERNAL_ERROR: i32 = -1;
    pub(crate) const MALFORMED_REQUEST: i32 = -2;
    pub(crate) const MALFORMED_RESPONSE: i32 = -3;
    pub(crate) const INVALID_METHOD: i32 = -4;
    pub(crate) const UNIMPLEMENTED_METHOD: i32 = -5;
    pub(crate) const INVALID_PARAMS: i32 = -6;
    pub(crate) const INVALID_RESULT: i32 = -7;
    pub(crate) const FORBIDDEN: i32 = -8;
    pub(crate) const TIMEOUT: i32 = -9;

    // Session error codes
    pub(crate) const SESSION_NOT_EXISTS: i32 = -101;
    pub(crate) const REVOKE_CURRENT_SESSION: i32 = -102;

    // Contact synchronization error codes
    pub(crate) const CONTACT_STORE_ERROR: i32 = -201;
    pub(crate) const REVISION_OUT_DATE: i32 = -202;
    pub(crate) const MALFORMED_MUTATION_DATA: i32 = -203;
    pub(crate) const CONTACT_NOT_EXISTS: i32 = -204;
    pub(crate) const CONTACT_ALREADY_EXISTS: i32 = -205;

    // Channel error codes
    pub(crate) const CHANNEL_NOT_EXISTS: i32 = -301;
    pub(crate) const CHANNEL_ALREADY_EXISTS: i32 = -302;
    pub(crate) const ALREADY_JOINED_CHANNEL: i32 = -303;
    pub(crate) const INVALID_INVITE_TICKET: i32 = -304;
    pub(crate) const FORBIDDEN_NON_CHANNEL_MEMBER: i32 = -305;
    pub(crate) const FORBIDDEN_BANNED_CHANNEL_MEMBER: i32 = -306;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RPCError {
    #[serde(rename = "c")]
    code: i32,

    #[serde(rename = "m")]
    message: String,

    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

#[allow(dead_code)]
impl RPCError {
    pub(crate) fn new(code: i32, message: &str, data: Option<String>) -> Self {
        Self {
            code,
            message: message.to_string(),
            data,
        }
    }

    /// Converts any error into a wire error, keeping the code of an `RPCError`.
    pub(crate) fn from_error(error: &(dyn Error + 'static)) -> Self {
        match error.downcast_ref::<RPCError>() {
            Some(error) => error.clone(),
            None => Self::new(code::RPC_INTERNAL_ERROR, &error.to_string(), None),
        }
    }

    pub(crate) fn code(&self) -> i32 {
        self.code
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }

    pub(crate) fn internal_error() -> Self {
        Self::new(code::RPC_INTERNAL_ERROR, "Super node internal error", None)
    }

    pub(crate) fn malformed_request() -> Self {
        Self::new(code::MALFORMED_REQUEST, "Malformed RPC request", None)
    }

    pub(crate) fn malformed_response() -> Self {
        Self::new(code::MALFORMED_RESPONSE, "Malformed RPC response", None)
    }

    pub(crate) fn invalid_method() -> Self {
        Self::new(code::INVALID_METHOD, "Invalid RPC method", None)
    }

    pub(crate) fn unimplemented_method() -> Self {
        Self::new(code::UNIMPLEMENTED_METHOD, "Unimplemented RPC method", None)
    }

    pub(crate) fn invalid_parameters() -> Self {
        Self::new(code::INVALID_PARAMS, "Invalid RPC parameters", None)
    }

    pub(crate) fn invalid_result() -> Self {
        Self::new(code::INVALID_RESULT, "Invalid RPC result", None)
    }

    pub(crate) fn forbidden() -> Self {
        Self::new(code::FORBIDDEN, "Forbidden", None)
    }

    pub(crate) fn timeout() -> Self {
        Self::new(code::TIMEOUT, "RPC call timed out", None)
    }

    pub(crate) fn session_not_exists() -> Self {
        Self::new(code::SESSION_NOT_EXISTS, "Session not exists", None)
    }

    pub(crate) fn revoke_current_session() -> Self {
        Self::new(
            code::REVOKE_CURRENT_SESSION,
            "Cannot revoke the current session",
            None,
        )
    }

    pub(crate) fn already_joined_channel() -> Self {
        Self::new(code::ALREADY_JOINED_CHANNEL, "Already joined channel", None)
    }

    pub(crate) fn forbidden_non_channel_member() -> Self {
        Self::new(
            code::FORBIDDEN_NON_CHANNEL_MEMBER,
            "Non-channel member is forbidden",
            None,
        )
    }

    pub(crate) fn forbidden_banned_channel_member() -> Self {
        Self::new(
            code::FORBIDDEN_BANNED_CHANNEL_MEMBER,
            "Banned channel member is forbidden",
            None,
        )
    }
}

impl fmt::Display for RPCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RPCError [code={}, message={}", self.code, self.message)?;
        if let Some(data) = &self.data {
            write!(f, ", data={data}")?;
        }
        write!(f, "]")
    }
}

impl Error for RPCError {}
