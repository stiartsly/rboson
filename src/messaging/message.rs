use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;

use crate::Id;
use crate::messaging::errors::{Error, Result};

// ---------------------------------------------------------------------------
// ContentType
// ---------------------------------------------------------------------------

/// MIME content-type constants.
pub mod content_type {
    pub const HEADER_NAME: &str = "Content-Type";
    pub const TEXT:        &str = "text/plain";
    pub const JSON:        &str = "application/json";
    pub const CBOR:        &str = "application/cbor";
    pub const IMAGE_JPEG:  &str = "image/jpeg";
    pub const IMAGE_PNG:   &str = "image/png";
    pub const IMAGE_WEBP:  &str = "image/webp";
    pub const AUDIO_AAC:   &str = "audio/aac";
    pub const AUDIO_MP3:   &str = "audio/mpeg";
    pub const AUDIO_WEBM:  &str = "audio/webm";
    pub const VIDEO_MP4:   &str = "video/mp4";
    pub const VIDEO_WEBM:  &str = "video/webm";
    pub const BINARY:      &str = "application/octet-stream";
}

// ---------------------------------------------------------------------------
// ContentDisposition
// ---------------------------------------------------------------------------

/// Whether a message body should be shown inline or as a downloadable attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentDisposition {
    /// Display the content inline.
    Inline,
    /// Offer the content as a download, with an optional filename.
    Attachment { filename: Option<String> },
}

impl ContentDisposition {
    /// An inline disposition with no filename.
    pub fn inline() -> Self {
        ContentDisposition::Inline
    }

    /// An inline disposition carrying a filename hint.
    pub fn inline_with_name(_filename: impl Into<String>) -> Self {
        ContentDisposition::Inline // simplified: ignore filename for inline
    }

    /// An attachment disposition.
    pub fn attachment(filename: impl Into<String>) -> Self {
        ContentDisposition::Attachment { filename: Some(filename.into()) }
    }

    /// The disposition type as a lowercase string (`"inline"` or `"attachment"`).
    pub fn type_str(&self) -> &'static str {
        match self {
            ContentDisposition::Inline       => "inline",
            ContentDisposition::Attachment { .. } => "attachment",
        }
    }

    /// The filename hint, if any.
    pub fn filename(&self) -> Option<&str> {
        match self {
            ContentDisposition::Attachment { filename } => filename.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentDisposition::Inline => f.write_str("inline"),
            ContentDisposition::Attachment { filename: Some(name) } => {
                write!(f, "attachment; filename=\"{}\"", name)
            },
            ContentDisposition::Attachment { filename: _ } => {
                f.write_str("attachment")
            },
        }
    }
}

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
    ContentMessage   = 1,
    /// Control / signalling message (not user-visible).
    ControlMessage   = 2,
    /// State-synchronisation message.
    StateMessage     = 3,
}

impl TryFrom<i32> for MessageType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(MessageType::HandshakeMessage),
            1 => Ok(MessageType::ContentMessage),
            2 => Ok(MessageType::ControlMessage),
            3 => Ok(MessageType::StateMessage),
            _ => Err(Error::Argument(format!("Unknown MessageType value: {}", value))),
        }
    }
}

// ---------------------------------------------------------------------------
// Message content
// ---------------------------------------------------------------------------

/// The decoded content of a [`Message`].
pub struct Content {
    headers:      HashMap<String, String>,
    content_type: Option<String>,
    disposition:  Option<ContentDisposition>,
    body:         Vec<u8>,
}

impl Content {
    pub(crate) fn _new(
        headers:      HashMap<String, String>,
        content_type: Option<String>,
        disposition:  Option<ContentDisposition>,
        body:         Vec<u8>,
    ) -> Self {
        Self { headers, content_type, disposition, body }
    }

    /// The raw header map.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// The MIME content type, defaulting to `text/plain` when absent.
    pub fn content_type(&self) -> &str {
        self.content_type.as_deref().unwrap_or(content_type::TEXT)
    }

    /// The content disposition, defaulting to `inline` when absent.
    pub fn content_disposition(&self) -> ContentDisposition {
        self.disposition.clone().unwrap_or(ContentDisposition::Inline)
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
    /// The local storage ID for this message.
    fn id(&self) -> i64;

    /// The ID of the conversation (= the other party's boson `Id`).
    fn conversation_id(&self) -> &Id;

    /// The intended recipient's boson `Id`.
    fn recipient(&self) -> Option<&Id>;

    /// The category of this message.
    fn message_type(&self) -> MessageType;

    /// The sender's boson `Id`.
    fn from(&self) -> &Id;

    /// When the message was authored.
    fn created_at(&self) -> SystemTime;

    /// When this device received the message (`None` for outbound messages).
    fn received_at(&self) -> Option<SystemTime>;

    /// When this device successfully sent the message (`None` for inbound).
    fn sent_at(&self) -> Option<SystemTime>;

    /// The raw encrypted payload bytes.
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
}
