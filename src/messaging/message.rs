use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::messaging::errors::{Error, Result};
use crate::Id;

// ---------------------------------------------------------------------------
// ContentType
// ---------------------------------------------------------------------------

/// MIME content-type constants.
pub mod content_type {
    pub const HEADER_NAME: &str = "Content-Type";
    pub const TEXT: &str = "text/plain";
    pub const JSON: &str = "application/json";
    pub const CBOR: &str = "application/cbor";
    pub const IMAGE_JPEG: &str = "image/jpeg";
    pub const IMAGE_PNG: &str = "image/png";
    pub const IMAGE_WEBP: &str = "image/webp";
    pub const AUDIO_AAC: &str = "audio/aac";
    pub const AUDIO_MP3: &str = "audio/mpeg";
    pub const AUDIO_WEBM: &str = "audio/webm";
    pub const VIDEO_MP4: &str = "video/mp4";
    pub const VIDEO_WEBM: &str = "video/webm";
    pub const BINARY: &str = "application/octet-stream";
}

// ---------------------------------------------------------------------------
// ContentDisposition
// ---------------------------------------------------------------------------

pub const CONTENT_DISPOSITION_HEADER: &str = "Content-Disposition";

/// How message content should be presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentDispositionType {
    Inline,
    Attachment,
}

impl ContentDispositionType {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "inline" => Ok(Self::Inline),
            "attachment" => Ok(Self::Attachment),
            _ => Err(Error::Argument(format!(
                "Invalid content disposition type: {value}"
            ))),
        }
    }
}

impl fmt::Display for ContentDispositionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inline => "inline",
            Self::Attachment => "attachment",
        })
    }
}

/// Parsed value of a `Content-Disposition` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDisposition {
    disposition_type: ContentDispositionType,
    ascii_filename: Option<String>,
    rfc5987_filename: Option<String>,
    filename: Option<String>,
}

impl ContentDisposition {
    /// An inline disposition with no filename.
    pub fn inline() -> Self {
        Self {
            disposition_type: ContentDispositionType::Inline,
            ascii_filename: None,
            rfc5987_filename: None,
            filename: None,
        }
    }

    /// An inline disposition carrying a filename hint.
    pub fn inline_with_name(filename: impl Into<String>) -> Self {
        Self::with_filename(ContentDispositionType::Inline, filename.into())
    }

    /// An attachment disposition.
    pub fn attachment(filename: impl Into<String>) -> Self {
        Self::with_filename(ContentDispositionType::Attachment, filename.into())
    }

    fn with_filename(disposition_type: ContentDispositionType, filename: String) -> Self {
        if filename.is_empty() {
            return Self {
                disposition_type,
                ascii_filename: None,
                rfc5987_filename: None,
                filename: None,
            };
        }

        Self {
            disposition_type,
            ascii_filename: Some(ascii_fallback(&filename)),
            rfc5987_filename: Some(encode_rfc5987(&filename)),
            filename: Some(filename),
        }
    }

    pub fn parse(header: &str) -> Result<Self> {
        let mut parts = header.split(';');
        let disposition_type = ContentDispositionType::parse(
            &parts.next().unwrap_or_default().trim().to_ascii_lowercase(),
        )?;
        let mut ascii_filename = None;
        let mut rfc5987_filename = None;

        for part in parts {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            match name.trim().to_ascii_lowercase().as_str() {
                "filename" => {
                    ascii_filename = Some(unquote(value.trim()).to_string());
                }
                "filename*" => {
                    rfc5987_filename = Some(value.trim().to_string());
                }
                _ => {}
            }
        }

        let filename = match rfc5987_filename.as_deref() {
            Some(value) => Some(decode_rfc5987(value)?),
            None => ascii_filename.clone(),
        };

        Ok(Self {
            disposition_type,
            ascii_filename,
            rfc5987_filename,
            filename,
        })
    }

    pub fn disposition_type(&self) -> ContentDispositionType {
        self.disposition_type
    }

    /// The disposition type as a lowercase string (`"inline"` or `"attachment"`).
    pub fn type_str(&self) -> &'static str {
        match self.disposition_type {
            ContentDispositionType::Inline => "inline",
            ContentDispositionType::Attachment => "attachment",
        }
    }

    /// The filename hint, if any.
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    pub fn ascii_filename(&self) -> Option<&str> {
        self.ascii_filename.as_deref()
    }

    pub fn rfc5987_filename(&self) -> Option<&str> {
        self.rfc5987_filename.as_deref()
    }

    pub fn is_inline(&self) -> bool {
        self.disposition_type == ContentDispositionType::Inline
    }

    pub fn is_attachment(&self) -> bool {
        self.disposition_type == ContentDispositionType::Attachment
    }

    pub fn value(&self) -> String {
        let mut value = self.disposition_type.to_string();
        if let Some(filename) = &self.ascii_filename {
            value.push_str(&format!("; filename=\"{filename}\""));
        }
        if let Some(filename) = &self.rfc5987_filename {
            value.push_str(&format!("; filename*={filename}"));
        }
        value
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{CONTENT_DISPOSITION_HEADER}: {}", self.value())
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn ascii_fallback(filename: &str) -> String {
    filename
        .nfkd()
        .filter(|ch| !is_combining_mark(*ch))
        .map(|ch| {
            if ch.is_ascii() && !ch.is_ascii_control() {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn encode_rfc5987(filename: &str) -> String {
    let mut encoded = String::from("UTF-8''");
    for byte in filename.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn decode_rfc5987(value: &str) -> Result<String> {
    let (charset, encoded) = value
        .split_once('\'')
        .and_then(|(charset, rest)| rest.strip_prefix('\'').map(|encoded| (charset, encoded)))
        .ok_or_else(|| Error::Argument(format!("Invalid RFC 5987 content disposition: {value}")))?;
    if !charset.eq_ignore_ascii_case("utf-8") {
        return Err(Error::Argument(format!(
            "Unsupported RFC 5987 charset: {charset}"
        )));
    }

    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos] == b'%' {
            if pos + 2 >= bytes.len() {
                return Err(Error::Argument(format!(
                    "Invalid RFC 5987 content disposition: {value}"
                )));
            }
            let hex = std::str::from_utf8(&bytes[pos + 1..pos + 3])
                .map_err(|_| Error::Argument("Invalid RFC 5987 escape".into()))?;
            decoded.push(u8::from_str_radix(hex, 16).map_err(|_| {
                Error::Argument(format!("Invalid RFC 5987 content disposition: {value}"))
            })?);
            pos += 3;
        } else {
            decoded.push(bytes[pos]);
            pos += 1;
        }
    }
    String::from_utf8(decoded)
        .map_err(|_| Error::Argument(format!("Invalid RFC 5987 content disposition: {value}")))
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
            _ => Err(Error::Argument(format!(
                "Unknown MessageType value: {}",
                value
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Message content
// ---------------------------------------------------------------------------

/// The decoded content of a [`Message`].
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
        self.headers
            .get(CONTENT_DISPOSITION_HEADER)
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| {
                        Error::Argument("Content-Disposition header must be a string".into())
                    })
                    .and_then(ContentDisposition::parse)
            })
            .transpose()
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
}

#[cfg(test)]
mod tests {
    use super::ContentDisposition;

    #[test]
    fn parses_rfc5987_filename() {
        let disposition = ContentDisposition::parse(
            "attachment; filename=\"file_name.jpg\"; filename*=UTF-8''file%20name.jpg",
        )
        .unwrap();

        assert!(disposition.is_attachment());
        assert_eq!(disposition.filename(), Some("file name.jpg"));
        assert_eq!(disposition.ascii_filename(), Some("file_name.jpg"));
        assert_eq!(
            disposition.rfc5987_filename(),
            Some("UTF-8''file%20name.jpg")
        );
    }

    #[test]
    fn creates_unicode_attachment() {
        let disposition = ContentDisposition::attachment("fïle nãme.jpg");

        assert_eq!(disposition.filename(), Some("fïle nãme.jpg"));
        assert_eq!(disposition.ascii_filename(), Some("file name.jpg"));
        assert_eq!(
            disposition.value(),
            "attachment; filename=\"file name.jpg\"; filename*=UTF-8''f%C3%AFle%20n%C3%A3me.jpg"
        );
    }
}
