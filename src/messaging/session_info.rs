use std::fmt;

use crate::Id;
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
