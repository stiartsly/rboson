use serde::{Deserialize, Serialize};

use crate::messaging::errors::EncodingError;
use crate::{
    errors::{Error, Result},
    Id,
};

/// Friend-request handshake payload.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "y", content = "b")]
pub(crate) enum HandshakeType {
    #[serde(rename = "fr")]
    FriendRequest(Option<String>),
    #[serde(rename = "fra")]
    FriendRequestAccept(
        #[serde(with = "serde_with::As::<serde_with::Bytes>")] Vec<u8>,
    ),
}

/// Signed friend handshake envelope.
#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "HandshakeData")]
pub(crate) struct Handshake {
    #[serde(rename = "t")]
    pub(crate) timestamp: i64,

    #[serde(flatten)]
    pub(crate) body: HandshakeType,

    /// Sender identity, filled in by the transport layer.
    #[serde(skip)]
    pub(crate) from: Option<Id>,
}

#[derive(Deserialize)]
struct HandshakeData {
    #[serde(rename = "t")]
    timestamp: i64,
    #[serde(flatten)]
    body: HandshakeType,
}

impl TryFrom<HandshakeData> for Handshake {
    type Error = Error;

    fn try_from(data: HandshakeData) -> Result<Self> {
        Self::new(data.timestamp, data.body)
    }
}

impl Handshake {
    pub(crate) fn new(timestamp: i64, body: HandshakeType) -> Result<Self> {
        if timestamp < 0 {
            return Err(EncodingError::new("Handshake timestamp is negative"));
        }
        if let HandshakeType::FriendRequestAccept(key) = &body {
            if key.len() != crate::signature::PrivateKey::BYTES {
                return Err(EncodingError::new(format!(
                    "Invalid friend session key length: {}",
                    key.len()
                )));
            }
        }
        Ok(Self {
            timestamp,
            body,
            from: None,
        })
    }
}

#[cfg(test)]
#[path = "../unitests/test_handshake.rs"]
mod test_handshake;
