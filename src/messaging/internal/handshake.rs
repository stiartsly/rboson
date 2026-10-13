use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use crate::{Id, errors::{Error, Result}};
use crate::messaging::errors::EncodingError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub(crate) enum HandshakeType {
    #[serde(rename = "fr")]
    FriendRequest = 1,
    #[serde(rename = "fra")]
    FriendRequestAccept = 2,
}

impl TryFrom<i32> for HandshakeType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            1 => Ok(Self::FriendRequest),
            2 => Ok(Self::FriendRequestAccept),
            _ => Err(EncodingError::new(format!("Invalid handshake type: {value}"))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Handshake {
    #[serde(rename = "t")]
    pub(crate) timestamp: i64,

    #[serde(rename = "y")]
    pub(crate) handshake_type: HandshakeType,

    #[serde(
        rename = "b",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) body: Option<Value>,

    #[serde(skip)]
    pub(crate) from: Option<Id>,
}

impl Handshake {
    pub(crate) fn new(
        timestamp: i64,
        handshake_type: HandshakeType,
        body: Value
    ) -> Result<Self> {
        let handshake = Self {
            timestamp,
            handshake_type,
            body: Some(body),
            from: None
        };
        handshake.validate()?;
        Ok(handshake)
    }

    fn validate(&self) -> Result<()> {
        if self.timestamp < 0 {
            return Err(EncodingError::new("Handshake timestamp is negative"));
        }
        match (self.handshake_type, self.body.as_ref()) {
            (HandshakeType::FriendRequest, None | Some(Value::Text(_))) => Ok(()),
            (HandshakeType::FriendRequestAccept, Some(Value::Bytes(key)))
                if key.len() == crate::signature::PrivateKey::BYTES => Ok(()),
            _ => Err(EncodingError::new("Handshake body does not match its type")),
        }
    }
    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding handshake failed: {error}")))?)
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let handshake: Self = serde_cbor::from_slice(bytes)
            .map_err(|error| EncodingError::new(format!("Malformed handshake: {error}")))?;
        handshake.validate()?;
        Ok(handshake)
    }
}
