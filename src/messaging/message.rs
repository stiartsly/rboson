use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    time::SystemTime,
};
use crate::errors::{Error, Result, ArgumentError};
use crate::Id;

pub use super::content_type;
pub use super::content_disposition::{
    ContentDisposition, Type, HEADER_NAME as CONTENT_DISPOSITION_HEADER,
};

// ---------------------------------------------------------------------------
// MessageType
// ---------------------------------------------------------------------------

/// Top-level category of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum MessageType {
    /// Initial handshake / key-exchange message.
    HandshakeMessage = 0,
    /// Regular user-visible content message.
    ContentMessage = 1,
    /// Control / signalling message (not user-visible).
    ControlMessage = 2,
    /// State-synchronisation message.
    StateMessage = 3,
}

impl TryFrom<i32> for MessageType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(MessageType::HandshakeMessage),
            1 => Ok(MessageType::ContentMessage),
            2 => Ok(MessageType::ControlMessage),
            3 => Ok(MessageType::StateMessage),
            _ => Err(ArgumentError::new(format!("Unknown MessageType value: {}",value)))
        }
    }
}

// ---------------------------------------------------------------------------
// Message content
// ---------------------------------------------------------------------------

/// The decoded content of a [`Message`].
#[derive(Clone)]
pub struct Content {
    headers: HashMap<String, serde_json::Value>,
    body: Vec<u8>,
}

impl Content {
    pub(crate) fn _new(headers: HashMap<String, serde_json::Value>, body: Vec<u8>) -> Self {
        Self { headers, body }
    }

    /// The raw header map.
    pub fn headers(&self) -> &HashMap<String, serde_json::Value> {
        &self.headers
    }

    /// The MIME content type, defaulting to `text/plain` when absent.
    pub fn content_type(&self) -> &str {
        self.headers
            .get("Content-Type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(content_type::TEXT)
    }

    /// The parsed content disposition, if the header is present and valid.
    pub fn content_disposition(&self) -> Result<Option<ContentDisposition>> {
        let Some(value) = self.headers.get(CONTENT_DISPOSITION_HEADER) else {
            return Ok(None);
        };
        let value = value.as_str().ok_or_else(|| {
            ArgumentError::new("Content-Disposition header must be a string")
        })?;
        Ok(Some(ContentDisposition::parse(value)?))
    }

    /// The raw body bytes.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Attempt to decode the body as a UTF-8 text string.
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }

    /// The body bytes as a `Vec<u8>`.
    pub fn as_binary(&self) -> Vec<u8> {
        self.body.clone()
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A single message in a conversation.
pub trait Message: Send + Sync {
    /// The globally unique sender-generated message ID.
    fn id(&self) -> &Id;

    /// The device-local, store-assigned row ID, or zero before persistence.
    fn rid(&self) -> i64;

    /// The conversation ID, if assigned.
    fn conversation_id(&self) -> Option<&Id>;

    /// The intended recipient's boson `Id`.
    fn recipient(&self) -> &Id;

    /// The category of this message.
    fn message_type(&self) -> MessageType;

    /// The sender's boson `Id`, if the message has been stamped for dispatch.
    fn from(&self) -> Option<&Id>;

    /// When the message was authored.
    fn created_at(&self) -> SystemTime;

    /// When this device received the message (`None` for outbound messages).
    fn received_at(&self) -> Option<SystemTime>;

    /// When this device successfully sent the message (`None` for inbound).
    fn sent_at(&self) -> Option<SystemTime>;

    /// The raw payload bytes.
    fn payload_as_bytes(&self) -> &[u8];

    /// The decoded content, if decryption succeeded.
    fn payload_as_content(&self) -> Option<&Content>;
}

// ---------------------------------------------------------------------------
// MessageBuilder trait
// ---------------------------------------------------------------------------

/// Fluent builder for composing and sending a message.
///
/// Methods that accept a `String`-like value take `&str` to remain dyn-compatible.
pub trait MessageBuilder: Send + Sync {
    /// Set the MIME content type.
    fn content_type(self: Box<Self>, ct: &str) -> Box<dyn MessageBuilder>;

    /// Set the content disposition.
    fn content_disposition(self: Box<Self>, cd: ContentDisposition) -> Box<dyn MessageBuilder>;

    /// Set a UTF-8 text body.
    fn text_body(self: Box<Self>, text: &str) -> Box<dyn MessageBuilder>;

    /// Set an arbitrary binary body.
    fn binary_body(self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilder>;

    /// Add an arbitrary header.
    fn header(self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilder>;

    /// Builds and sends the message.
    fn send(
        self: Box<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Message>>> + Send + 'static>>;
}
