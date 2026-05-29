use serde::{Deserialize, Serialize};
use crate::Id;

/// Information about a single device session for the current user.
///
/// CBOR field names match the Java `SessionInfo` record:
/// `id` = device_id, `o` = online, `lt` = last_active_ms, `la` = last_address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The boson `Id` of the device.
    #[serde(rename = "id")]
    pub device_id: Id,

    /// Whether this device is currently online.
    #[serde(rename = "o")]
    pub online: bool,

    /// Timestamp (milliseconds since UNIX epoch) of the last activity.
    #[serde(rename = "lt")]
    pub last_active_ms: i64,

    /// Last known network address (IP:port string), if available.
    #[serde(rename = "la", skip_serializing_if = "Option::is_none")]
    pub last_address: Option<String>,
}

impl SessionInfo {
    pub fn new(
        device_id:     Id,
        online:        bool,
        last_active_ms: i64,
        last_address:  Option<String>,
    ) -> Self {
        Self { device_id, online, last_active_ms, last_address }
    }

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
