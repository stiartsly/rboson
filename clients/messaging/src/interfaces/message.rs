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
    future::Future,
    pin::Pin,
    time::SystemTime,
};

use boson_api::{Id, Result};
use crate::{
    content_disposition::ContentDisposition,
    content_type,
    MessageType,
};

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
    fn payload_as_content(&self) -> Option<Box<dyn MessageContent>>;

    /// Clones this message into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn Message>;
}

impl Clone for Box<dyn Message> {
    fn clone(&self) -> Self {
        Message::clone_box(&**self)
    }
}

/// Accessors for decoded message content.
pub trait MessageContent: Send + Sync {
    /// The content headers.
    fn headers(&self) -> HashMap<String, serde_json::Value>;

    /// The body bytes.
    fn body(&self) -> Vec<u8>;

    /// The MIME content type, defaulting to `text/plain`.
    fn content_type(&self) -> String {
        self.headers()
            .get(content_type::HEADER_NAME)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(content_type::TEXT)
            .to_string()
    }

    /// The parsed content disposition, if present.
    fn content_disposition(&self) -> Result<Option<ContentDisposition>> {
        let headers = self.headers();
        let Some(value) = headers.get(ContentDisposition::HEADER_NAME) else {
            return Ok(None);
        };
        let value = value
            .as_str()
            .ok_or_else(|| boson_api::errors::ArgumentError::new(
                "Content-Disposition header must be a string",
            ))?;
        Ok(Some(ContentDisposition::parse(value)?))
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
pub trait MessageBuilder: Send + Sync {
    /// Set the MIME content type.
    fn with_content_type(self: Box<Self>, ct: &str) -> Box<dyn MessageBuilder>;

    /// Set the content disposition.
    fn with_content_disposition(self: Box<Self>, cd: ContentDisposition) -> Box<dyn MessageBuilder>;

    /// Set a UTF-8 text body.
    fn with_text_body(self: Box<Self>, text: &str) -> Box<dyn MessageBuilder>;

    /// Set an arbitrary binary body.
    fn with_binary_body(self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilder>;

    /// Add an arbitrary header.
    fn with_header(self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilder>;

    /// Builds and sends the message.
    fn send(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<Box<dyn Message>>> + Send>>;
}
