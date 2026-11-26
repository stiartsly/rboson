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

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    time::SystemTime,
};

use boson_api::{
    errors::ArgumentError,
    Error,
    Id,
    Result,
};
use crate::{
    content_disposition::ContentDisposition,
    content_type::{self, ContentType},
};

// ---------------------------------------------------------------------------
// MessageType
// ---------------------------------------------------------------------------

/// Top-level category of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum MessageType {
    /// Initial handshake / key-exchange message.
    Handshake = 0,
    /// Regular user-visible content message.
    Content = 1,
    /// Control / signalling message (not user-visible).
    Control = 2,
    /// State-synchronisation message.
    State = 3,
}

impl MessageType {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(MessageType::Handshake),
            1 => Some(MessageType::Content),
            2 => Some(MessageType::Control),
            3 => Some(MessageType::State),
            _ => None,
        }
    }
}

impl TryFrom<i32> for MessageType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        Self::from_i32(value).ok_or_else(||
            ArgumentError::new(format!("invalid message type: {}", value)).into())
    }
}

impl AsRef<str> for MessageType {
    fn as_ref(&self) -> &str {
        match self {
            MessageType::Handshake => "handshake",
            MessageType::Content => "content",
            MessageType::Control => "control",
            MessageType::State => "state",
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// A single message in a conversation.
pub trait MessageHandler: Send + Sync {
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
    fn payload_as_content(&self) -> Option<Box<dyn MessageContentHandler>>;

    /// Clones this message into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn MessageHandler>;
}

impl Clone for Box<dyn MessageHandler> {
    fn clone(&self) -> Self {
        MessageHandler::clone_box(&**self)
    }
}

/// Accessors for decoded message content.
pub trait MessageContentHandler: Send + Sync {
    /// The content headers.
    fn headers(&self) -> HashMap<String, serde_json::Value>;

    /// The body bytes.
    fn body(&self) -> Vec<u8>;

    /// The MIME content type, defaulting to `text/plain`.
    fn content_type(&self) -> String {
        self.headers()
            .get(content_type::HEADER_NAME)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(ContentType::TextPlain.as_str())
            .to_string()
    }

    /// The parsed content disposition, if present.
    fn content_disposition(&self) -> Option<ContentDisposition> {
        let headers = self.headers();
        let Some(value) = headers.get(ContentDisposition::HEADER_NAME) else {
            return None;
        };
        let value = value.as_str()?;
        Some(ContentDisposition::parse(value).ok()?)
    }

    /// The body as UTF-8 text.
    fn as_text(&self) -> Result<String> {
        String::from_utf8(self.body())
            .map_err(|error| crate::errors::EncodingError::new(error.to_string()).into())
    }

    /// The body as binary data.
    fn as_binary(&self) -> Result<Vec<u8>> {
        Ok(self.body())
    }
}

/// Fluent builder for composing and sending a message.
pub trait MessageBuilderHandler: Send + Sync {
    /// Set the MIME content type.
    fn with_content_type(self: Box<Self>, ct: &str) -> Box<dyn MessageBuilderHandler>;

    /// Set the content disposition.
    fn with_content_disposition(self: Box<Self>, cd: ContentDisposition) -> Box<dyn MessageBuilderHandler>;

    /// Set a UTF-8 text body.
    fn with_text_body(self: Box<Self>, text: &str) -> Box<dyn MessageBuilderHandler>;

    /// Set an arbitrary binary body.
    fn with_binary_body(self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilderHandler>;

    /// Add an arbitrary header.
    fn with_header(self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilderHandler>;

    /// Builds and sends the message.
    fn send(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<Box<dyn MessageHandler>>> + Send>>;
}

pub type Message = Box<dyn MessageHandler>;
pub type MessageContent = Box<dyn MessageContentHandler>;
pub type MessageBuilder = Box<dyn MessageBuilderHandler>;
