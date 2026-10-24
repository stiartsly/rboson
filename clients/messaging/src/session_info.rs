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

use boson_api::Id;
use serde::{Deserialize, Serialize};

/// Information about a single device session for the current user.
///
/// CBOR field names match the Java `SessionInfo` record:
/// `id` = device_id, `o` = online, `lt` = last_active_ms, `la` = last_address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The boson `Id` of the device.
    #[serde(rename = "id")]
    device_id: Id,

    /// Whether this device is currently online.
    #[serde(rename = "o", default)]
    online: bool,

    /// Timestamp (milliseconds since UNIX epoch) of the last activity.
    #[serde(rename = "lt", default)]
    last_active_ms: i64,

    /// Last known network address (IP:port string), if available.
    #[serde(
        rename = "la",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    last_address: Option<String>,
}

impl SessionInfo {
    pub fn device_id(&self) -> &Id {
        &self.device_id
    }

    pub fn is_online(&self) -> bool {
        self.online
    }

    pub fn last_active_ms(&self) -> i64 {
        self.last_active_ms
    }

    pub fn last_address(&self) -> Option<&str> {
        self.last_address.as_deref()
    }
}

impl PartialEq for SessionInfo {
    fn eq(&self, other: &Self) -> bool {
        self.device_id == other.device_id
            && self.online == other.online
            && self.last_active_ms == other.last_active_ms
            && self.last_address == other.last_address
    }
}

impl fmt::Display for SessionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionInfo {{ device_id: {}, online: {}, last_active_ms: {}, last_address: {} }}",
            self.device_id,
            self.online,
            self.last_active_ms,
            self.last_address.as_deref().unwrap_or("N/A")
        )
    }
}
