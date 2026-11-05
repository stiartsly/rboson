use std::{
    collections::HashMap,
    future::Future,
    hash::Hash,
    time::SystemTime,
};

use serde::de::DeserializeOwned;

use boson_api::{Id, Result};
use crate::{
    content_disposition::ContentDisposition,
    content_type,
    Content as ContentStruct,
    Message as MessageStruct,
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
    fn payload_as_content(&self) -> Option<&ContentStruct>;
}

/// Accessors for decoded message content.
pub trait Content {
    /// The content headers.
    fn headers(&self) -> &HashMap<String, serde_json::Value>;

    /// The body bytes.
    fn body(&self) -> &[u8];

    /// The MIME content type, defaulting to `text/plain`.
    fn content_type(&self) -> &str {
        self.headers()
            .get(content_type::HEADER_NAME)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(content_type::TEXT)
    }

    /// The parsed content disposition, if present.
    fn content_disposition(&self) -> Result<Option<ContentDisposition>> {
        let Some(value) = self.headers().get(ContentDisposition::HEADER_NAME) else {
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
        String::from_utf8(self.body().to_vec())
            .map_err(|error| crate::errors::EncodingError::new(error.to_string()).into())
    }

    /// The body as binary data.
    fn as_binary(&self) -> Result<Vec<u8>> {
        Ok(self.body().to_vec())
    }

    /// Deserializes the JSON body into `T`.
    fn as_object<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(self.body())
            .map_err(|error| crate::errors::EncodingError::new(error.to_string()).into())
    }

    /// Deserializes the JSON body as a list.
    fn as_list<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.as_object()
    }

    /// Deserializes the JSON body as a map.
    fn as_map<K, V>(&self) -> Result<HashMap<K, V>>
    where
        K: DeserializeOwned + Eq + Hash,
        V: DeserializeOwned,
    {
        self.as_object()
    }
}

/// Fluent builder for composing and sending a message.
pub trait MessageBuilder: Send + Sync {
    /// Set the MIME content type.
    fn with_content_type(self, ct: &str) -> Self;

    /// Set the content disposition.
    fn with_content_disposition(self, cd: ContentDisposition) -> Self;

    /// Set a UTF-8 text body.
    fn with_text_body(self, text: &str) -> Self;

    /// Set an arbitrary binary body.
    fn with_binary_body(self, data: Vec<u8>) -> Self;

    /// Add an arbitrary header.
    fn with_header(self, key: &str, value: &str) -> Self;

    /// Builds and sends the message.
    fn send(self) -> impl Future<Output = Result<MessageStruct>>;
}
