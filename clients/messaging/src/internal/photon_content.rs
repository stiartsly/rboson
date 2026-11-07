// Rust implementation of the Java MessageContent wire type.

use std::{
    collections::HashMap,
    hash::Hash,
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use boson_api::{
    errors::ArgumentError,
    Result,
};

use crate::MessageContentOps;

/// Encoding of the message body carried by [`MessageContent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ContentFormat {
    #[serde(rename = "t")]
    Text,
    #[serde(rename = "b")]
    Binary,
    #[serde(rename = "o")]
    Object,
}

/// Decoded message content with format-specific accessors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PhotonContent {
    #[serde(
        rename = "h",
        default,
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub(crate) headers: HashMap<String, Value>,

    #[serde(rename = "f")]
    pub(crate) format: ContentFormat,

    #[serde(rename = "b")]
    pub(crate) body: Value,
}

impl Default for PhotonContent {
    fn default() -> Self {
        Self {
            headers: HashMap::new(),
            format: ContentFormat::Text,
            body: Value::Text(String::new()),
        }
    }
}

impl MessageContentOps for PhotonContent {
    fn headers(&self) -> HashMap<String, serde_json::Value> {
        self.headers
            .iter()
            .filter_map(|(key, value)| {
                serde_json::to_value(value).ok().map(|value| (key.clone(), value))
            })
            .collect()
    }

    fn body(&self) -> Vec<u8> {
        self.as_binary().unwrap_or_default()
    }
}

impl PhotonContent {
    pub(crate) fn new(
        headers: HashMap<String, Value>,
        format: ContentFormat,
        body: Value,
    ) -> Result<Self> {
        let valid = matches!(
            (&format, &body),
                (ContentFormat::Text, Value::Text(_))
                | (ContentFormat::Binary, Value::Bytes(_))
                | (ContentFormat::Object, _)
        );
        if !valid {
            return Err(ArgumentError::new(
                "Message body does not match its format",
            ));
        }
        Ok(Self {headers, format, body})
    }

    /// The empty, text/plain content.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Text content with headers.
    pub fn text_with_headers(
        headers: HashMap<String, Value>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            headers,
            format: ContentFormat::Text,
            body: Value::Text(text.into()),
        }
    }

    /// Text content without headers.
    pub fn text(text: impl Into<String>) -> Self {
        Self::text_with_headers(HashMap::new(), text)
    }

    /// Binary content with headers.
    pub fn binary_with_headers(
        headers: HashMap<String, Value>,
        binary: Vec<u8>,
    ) -> Self {
        Self {
            headers,
            format: ContentFormat::Binary,
            body: Value::Bytes(binary),
        }
    }

    /// Binary content without headers.
    pub fn binary(binary: Vec<u8>) -> Self {
        Self::binary_with_headers(HashMap::new(), binary)
    }

    /// Object content with headers.
    pub fn object_with_headers<T: Serialize>(
        headers: HashMap<String, Value>,
        object: &T,
    ) -> Result<Self> {
        let body = serde_cbor::value::to_value(object)
            .map_err(|error| ArgumentError::new(error.to_string()))?;
        Self::new(headers, ContentFormat::Object, body)
    }

    /// Object content without headers.
    pub fn object<T: Serialize>(object: &T) -> Result<Self> {
        Self::object_with_headers(HashMap::new(), object)
    }

    /// The metadata headers.
    pub fn headers(&self) -> &HashMap<String, Value> {
        &self.headers
    }

    /// The content body format.
    pub fn format(&self) -> ContentFormat {
        self.format
    }

    /// The raw body value.
    pub fn body(&self) -> &Value {
        &self.body
    }

    /// The body as text: text as-is, binary as UTF-8, object as JSON.
    pub fn as_text(&self) -> Result<String> {
        match (&self.format, &self.body) {
            (ContentFormat::Text, Value::Text(text)) => Ok(text.clone()),
            (ContentFormat::Binary, Value::Bytes(bytes)) => {
                String::from_utf8(bytes.clone()).map_err(|error| {
                    ArgumentError::new(error.to_string()).into()
                })
            }
            (ContentFormat::Object, body) => {
                let value: serde_json::Value = serde_cbor::value::from_value(body.clone())
                    .map_err(|error| ArgumentError::new(error.to_string()))?;
                serde_json::to_string(&value)
                    .map_err(|error| ArgumentError::new(error.to_string()).into())
            }
            _ => Err(ArgumentError::new(
                "Message body does not match its format",
            )
            .into()),
        }
    }

    /// The body as binary: text as UTF-8, binary as-is, object as CBOR.
    pub fn as_binary(&self) -> Result<Vec<u8>> {
        match (&self.format, &self.body) {
            (ContentFormat::Text, Value::Text(text)) => Ok(text.as_bytes().to_vec()),
            (ContentFormat::Binary, Value::Bytes(bytes)) => Ok(bytes.clone()),
            (ContentFormat::Object, body) => serde_cbor::to_vec(body)
                .map_err(|error| ArgumentError::new(error.to_string()).into()),
            _ => Err(ArgumentError::new(
                "Message body does not match its format",
            )
            .into()),
        }
    }

    /// Deserializes an object body to `T`.
    pub fn as_object<T: DeserializeOwned>(&self) -> Result<T> {
        self.ensure_object_format()?;
        serde_cbor::value::from_value(self.body.clone())
            .map_err(|error| ArgumentError::new(error.to_string()).into())
    }

    /// Deserializes an object body to a list.
    pub fn as_list<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.as_object()
    }

    /// Deserializes an object body to a map.
    pub fn as_map<K: DeserializeOwned + Hash + Eq, V: DeserializeOwned>(
        &self,
    ) -> Result<HashMap<K, V>> {
        self.as_object()
    }

    /// Serializes this content into CBOR.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self)
            .map_err(|error| ArgumentError::new(error.to_string()).into())
    }

    /// Parses CBOR content.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let content: Self = serde_cbor::from_slice(data)
            .map_err(|error| ArgumentError::new(error.to_string()))?;
        Self::new(content.headers, content.format, content.body)
    }

    fn ensure_object_format(&self) -> Result<()> {
        if self.format != ContentFormat::Object {
            return Err(ArgumentError::new(
                "Message content is not an object",
            )
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../unitests/test_content.rs"]
mod test_content;
