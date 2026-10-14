use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde::de::DeserializeOwned;
use serde_cbor::Value;
use serde::Serialize as SerdeSerialize;

use crate::errors::Result;
use crate::messaging::errors::EncodingError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    #[serde(rename = "t")]
    Text,
    #[serde(rename = "b")]
    Binary,
    #[serde(rename = "o")]
    Object,
}

pub(crate) type ContentFormat = Format;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MessageContent {
    #[serde(rename = "h", default, skip_serializing_if = "HashMap::is_empty")]
    pub(crate) headers: HashMap<String, Value>,
    #[serde(rename = "f")]
    pub(crate) format: ContentFormat,
    #[serde(rename = "b")]
    pub(crate) body: Value,
}

#[derive(Deserialize)]
struct MessageContentData {
    #[serde(rename = "h", default)]
    headers: HashMap<String, Value>,
    #[serde(rename = "f")]
    format: ContentFormat,
    #[serde(rename = "b")]
    body: Value,
}

impl<'de> Deserialize<'de> for MessageContent {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = MessageContentData::deserialize(deserializer)?;
        Self::new(data.headers, data.format, data.body)
            .map_err(serde::de::Error::custom)
    }
}

#[allow(dead_code)]
impl MessageContent {
    pub fn empty() -> Self {
        Self {
            headers: HashMap::new(),
            format: Format::Text,
            body: Value::Text(String::new()),
        }
    }

    pub fn new(
        headers: HashMap<String, Value>,
        format: Format,
        body: Value,
    ) -> Result<Self> {
        let valid = matches!(
            (&format, &body),
            (Format::Text, Value::Text(_))
                | (Format::Binary, Value::Bytes(_))
                | (Format::Object, _)
        );
        if !valid {
            return Err(EncodingError::new("Message body does not match its format"));
        }
        Ok(Self { headers, format, body })
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::text_with_headers(HashMap::new(), text)
    }

    pub fn text_with_headers(headers: HashMap<String, Value>, text: impl Into<String>) -> Self {
        Self {
            headers,
            format: Format::Text,
            body: Value::Text(text.into()),
        }
    }

    pub fn binary(binary: Vec<u8>) -> Self {
        Self::binary_with_headers(HashMap::new(), binary)
    }

    pub fn binary_with_headers(headers: HashMap<String, Value>, binary: Vec<u8>) -> Self {
        Self {
            headers,
            format: Format::Binary,
            body: Value::Bytes(binary),
        }
    }

    pub fn object<T: SerdeSerialize>(object: &T) -> Result<Self> {
        Self::object_with_headers(HashMap::new(), object)
    }

    pub fn object_with_headers<T: SerdeSerialize>(
        headers: HashMap<String, Value>,
        object: &T,
    ) -> Result<Self> {
        let body = serde_cbor::value::to_value(object)
            .map_err(|error| EncodingError::new(format!("Encoding message object failed: {error}")))?;
        Self::new(headers, Format::Object, body)
    }

    pub fn headers(&self) -> &HashMap<String, Value> {
        &self.headers
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn body(&self) -> &Value {
        &self.body
    }

    pub fn as_text(&self) -> Result<String> {
        match (&self.format, &self.body) {
            (Format::Text, Value::Text(text)) => Ok(text.clone()),
            (Format::Binary, Value::Bytes(bytes)) => String::from_utf8(bytes.clone())
                .map_err(|error| EncodingError::new(format!("Message body is not UTF-8: {error}")).into()),
            (Format::Object, body) => {
                let value: serde_json::Value = serde_cbor::value::from_value(body.clone())
                    .map_err(|error| EncodingError::new(format!("Decoding message object failed: {error}")))?;
                serde_json::to_string(&value)
                    .map_err(|error| EncodingError::new(format!("Encoding message object as JSON failed: {error}")).into())
            }
            _ => Err(EncodingError::new("Message body does not match its format").into()),
        }
    }

    pub fn as_binary(&self) -> Result<Vec<u8>> {
        match (&self.format, &self.body) {
            (Format::Text, Value::Text(text)) => Ok(text.as_bytes().to_vec()),
            (Format::Binary, Value::Bytes(bytes)) => Ok(bytes.clone()),
            (Format::Object, body) => serde_cbor::to_vec(body)
                .map_err(|error| EncodingError::new(format!("Encoding message object failed: {error}")).into()),
            _ => Err(EncodingError::new("Message body does not match its format").into()),
        }
    }

    pub fn as_object<T: DeserializeOwned>(&self) -> Result<T> {
        self.ensure_object_format()?;
        serde_cbor::value::from_value(self.body.clone())
            .map_err(|error| EncodingError::new(format!("Decoding message object failed: {error}")).into())
    }

    pub fn as_list<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.as_object()
    }

    pub fn as_map<K: DeserializeOwned, V: DeserializeOwned>(&self) -> Result<HashMap<K, V>>
    where
        K: std::hash::Hash + Eq,
    {
        self.as_object()
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Serializing message content failed: {error}")).into())
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data)
            .map_err(|error| EncodingError::new(format!("Parsing message content failed: {error}")).into())
    }

    fn ensure_object_format(&self) -> Result<()> {
        if self.format != Format::Object {
            return Err(EncodingError::new("Message content is not an object").into());
        }
        Ok(())
    }
}
