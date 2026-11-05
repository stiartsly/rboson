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
    sync::Arc,
    time::{SystemTime, Duration, UNIX_EPOCH},
};
use serde::{Serialize, Deserialize};
use serde_cbor::Value;

use boson_api::{
    errors::{ArgumentError,StateError},
    Error,
    Result,
    utils,
    Id,
};
use crate::{
    verticle,
    MessageListener,
    MessageOps,
    MessageBuilderOps,
};
use super::content_type;
pub use super::content_disposition::{
    ContentDisposition,
    Type
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
        let Some(value) = self.headers.get(ContentDisposition::HEADER_NAME) else {
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
#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "v")]
    pub(crate) version: u8,

    #[serde(rename = "id")]
    pub(crate) id: Id,

    #[serde(rename = "r")]
    pub(crate) recipient: Id,

    #[serde(rename = "y")]
    pub(crate) message_type: u8,

    #[serde(rename = "f", default, with = "utils::id_opt")]
    pub(crate) from: Option<Id>,

    #[serde(
        rename = "c",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) created_at: u64,

    #[serde(rename = "p")]
    pub(crate) payload: Value,

    #[serde(skip)]
    pub(crate) received_at: Option<SystemTime>,

    #[serde(skip)]
    pub(crate) sent_at: Option<SystemTime>,

    #[serde(skip)]
    pub(crate) payload_bytes: Vec<u8>,

    #[serde(skip)]
    pub(crate) content: Option<Content>,

    #[serde(skip)]
    pub(crate) conversation_id: Option<Id>,
}

impl Message {
    pub(crate) fn new(
        version: u8,
        id: Id,
        recipient: Id,
        message_type: u8,
        from: Option<Id>,
        created_at: u64,
        payload: Value,
    ) -> Self {
        Self {
            version,
            id,
            recipient,
            message_type,
            from,
            created_at,
            payload,
            received_at: None,
            sent_at: None,
            payload_bytes: Vec::new(),
            content: None,
            conversation_id: None,
        }
    }
}

impl MessageOps for Message {
    fn id(&self) -> &Id {
        &self.id
    }

    fn rid(&self) -> i64 {
        0
    }

    fn conversation_id(&self) -> Option<&Id> {
        self.conversation_id.as_ref().or_else(|| {
            if self.received_at.is_some() {
                self.from.as_ref()
            } else {
                Some(&self.recipient)
            }
        })
    }

    fn recipient(&self) -> &Id {
        &self.recipient
    }

    fn message_type(&self) -> MessageType {
        // Only content messages are surfaced through the public listener API.
        MessageType::ContentMessage
    }

    fn from(&self) -> Option<&Id> {
        self.from.as_ref()
    }

    fn created_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.created_at)
    }

    fn received_at(&self) -> Option<SystemTime> {
        self.received_at
    }

    fn sent_at(&self) -> Option<SystemTime> {
        self.sent_at
    }

    fn payload_as_bytes(&self) -> &[u8] {
        &self.payload_bytes
    }

    fn payload_as_content(&self) -> Option<&Content> {
        self.content.as_ref()
    }
}

pub struct MessageBuilder {
    recipient: Option<Id>,
    verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::Event>>,
    message_listener: Arc<dyn MessageListener>,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    text: bool,
}

impl MessageBuilder {
    pub(crate) fn new(
        recipient: Option<Id>,
        verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::Event>>,
        message_listener: Arc<dyn MessageListener>,
    ) -> Self {
        Self {
            recipient,
            verticle_tx,
            message_listener,
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        }
    }
}


impl MessageBuilderOps for MessageBuilder {
    fn with_content_type(mut self, content_type: &str) -> Self {
        self.content_type = Some(content_type.to_string());
        self
    }

    fn with_content_disposition(mut self, disposition: ContentDisposition) -> Self {
        self.content_disposition = Some(disposition);
        self
    }

    fn with_text_body(mut self, text: &str) -> Self {
        self.body = Some(text.as_bytes().to_vec());
        self.text = true;
        self
    }

    fn with_binary_body(mut self, data: Vec<u8>) -> Self {
        self.body = Some(data);
        self.text = false;
        self
    }

    fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    async fn send(self) -> Result<Message> {
        let recipient = self
            .recipient
            .ok_or_else(|| ArgumentError::new("Message recipient is required"))?;
        let body = self
            .body
            .ok_or_else(|| ArgumentError::new("Message content is required"))?;
        let sender = self
            .verticle_tx
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;

        let text = self.text;
        let headers = merge_headers(
            self.headers,
            self.content_type,
            self.content_disposition,
            text,
        );

        let (promise, future) = verticle::Promise::pair();
        sender
            .send(verticle::Event::ContentMessage(
                verticle::WithParams {
                    data: (recipient, headers, body, text),
                    promise
                },
            ))
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;

        let message = future.await.map_err(StateError::new)?;
        self.message_listener.on_sent(&message);
        Ok(message)
    }
}

fn merge_headers(
    headers: Vec<(String, String)>,
    declared_type: Option<String>,
    disposition: Option<ContentDisposition>,
    text: bool,
) -> HashMap<String, serde_json::Value> {
    let mut merged: HashMap<String, serde_json::Value> = headers
        .into_iter()
        .map(|(key, value)| (key, serde_json::Value::String(value)))
        .collect();

    if let Some(declared_type) = declared_type {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(declared_type),
        );
    } else if !text {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(content_type::BINARY.into()),
        );
    }

    if let Some(disposition) = disposition {
        merged.insert(
            ContentDisposition::HEADER_NAME.into(),
            serde_json::Value::String(disposition.value()),
        );
    }

    merged
}

#[cfg(test)]
#[path = "unitests/test_message.rs"]
mod test_message;
