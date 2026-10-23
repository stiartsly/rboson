use std::collections::HashMap;
#[cfg(test)]
use std::hash::Hash;

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use crate::errors::Result;
use crate::messaging::errors::EncodingError;

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
#[serde(try_from = "MessageContentData")]
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

impl TryFrom<MessageContentData> for MessageContent {
    type Error = crate::Error;

    fn try_from(data: MessageContentData) -> Result<Self> {
        Self::new(data.headers, data.format, data.body)
    }
}

impl MessageContent {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            headers: HashMap::new(),
            format: ContentFormat::Text,
            body: Value::Text(String::new()),
        }
    }

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
            return Err(EncodingError::new("Message body does not match its format"));
        }
        Ok(Self {
            headers,
            format,
            body,
        })
    }

    #[cfg(test)]
    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self::text_with_headers(HashMap::new(), text)
    }

    #[cfg(test)]
    pub(crate) fn text_with_headers(
        headers: HashMap<String, Value>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            headers,
            format: ContentFormat::Text,
            body: Value::Text(text.into()),
        }
    }

    #[cfg(test)]
    pub(crate) fn binary(binary: Vec<u8>) -> Self {
        Self::binary_with_headers(HashMap::new(), binary)
    }

    #[cfg(test)]
    pub(crate) fn binary_with_headers(
        headers: HashMap<String, Value>,
        binary: Vec<u8>,
    ) -> Self {
        Self {
            headers,
            format: ContentFormat::Binary,
            body: Value::Bytes(binary),
        }
    }

    #[cfg(test)]
    pub(crate) fn object<T: Serialize>(object: &T) -> Result<Self> {
        Self::object_with_headers(HashMap::new(), object)
    }

    #[cfg(test)]
    pub(crate) fn object_with_headers<T: Serialize>(
        headers: HashMap<String, Value>,
        object: &T,
    ) -> Result<Self> {
        let body = serde_cbor::value::to_value(object).map_err(|error| {
            EncodingError::new(format!("Encoding message object failed: {error}"))
        })?;
        Self::new(headers, ContentFormat::Object, body)
    }

    #[cfg(test)]
    pub(crate) fn headers(&self) -> &HashMap<String, Value> {
        &self.headers
    }

    #[cfg(test)]
    pub(crate) fn format(&self) -> ContentFormat {
        self.format
    }

    #[cfg(test)]
    pub(crate) fn body(&self) -> &Value {
        &self.body
    }

    #[cfg(test)]
    pub(crate) fn as_text(&self) -> Result<String> {
        match (&self.format, &self.body) {
            (ContentFormat::Text, Value::Text(text)) => Ok(text.clone()),
            (ContentFormat::Binary, Value::Bytes(bytes)) => {
                String::from_utf8(bytes.clone()).map_err(|error| {
                    EncodingError::new(format!("Message body is not UTF-8: {error}")).into()
                })
            }
            (ContentFormat::Object, body) => {
                let value: serde_json::Value =
                    serde_cbor::value::from_value(body.clone()).map_err(|error| {
                        EncodingError::new(format!("Decoding message object failed: {error}"))
                    })?;
                serde_json::to_string(&value).map_err(|error| {
                    EncodingError::new(format!(
                        "Encoding message object as JSON failed: {error}"
                    ))
                    .into()
                })
            }
            _ => Err(EncodingError::new("Message body does not match its format").into()),
        }
    }

    #[cfg(test)]
    pub(crate) fn as_binary(&self) -> Result<Vec<u8>> {
        match (&self.format, &self.body) {
            (ContentFormat::Text, Value::Text(text)) => Ok(text.as_bytes().to_vec()),
            (ContentFormat::Binary, Value::Bytes(bytes)) => Ok(bytes.clone()),
            (ContentFormat::Object, body) => serde_cbor::to_vec(body).map_err(|error| {
                EncodingError::new(format!("Encoding message object failed: {error}")).into()
            }),
            _ => Err(EncodingError::new("Message body does not match its format").into()),
        }
    }

    #[cfg(test)]
    pub(crate) fn as_object<T: DeserializeOwned>(&self) -> Result<T> {
        self.ensure_object_format()?;
        serde_cbor::value::from_value(self.body.clone()).map_err(|error| {
            EncodingError::new(format!("Decoding message object failed: {error}")).into()
        })
    }

    #[cfg(test)]
    pub(crate) fn as_list<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.as_object()
    }

    #[cfg(test)]
    pub(crate) fn as_map<K: DeserializeOwned + Hash + Eq, V: DeserializeOwned>(
        &self,
    ) -> Result<HashMap<K, V>> {
        self.as_object()
    }

    #[cfg(test)]
    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            EncodingError::new(format!("Serializing message content failed: {error}")).into()
        })
    }

    #[cfg(test)]
    pub(crate) fn parse(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|error| {
            EncodingError::new(format!("Parsing message content failed: {error}")).into()
        })
    }

    #[cfg(test)]
    fn ensure_object_format(&self) -> Result<()> {
        if self.format != ContentFormat::Object {
            return Err(EncodingError::new("Message content is not an object").into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers() -> HashMap<String, Value> {
        HashMap::from([("X-Test".to_string(), Value::Text("value".to_string()))])
    }

    #[test]
    fn constructors_match_their_formats() {
        let text = MessageContent::text_with_headers(headers(), "hello");
        assert_eq!(text.format(), ContentFormat::Text);
        assert_eq!(text.as_text().unwrap(), "hello");
        assert_eq!(text.as_binary().unwrap(), b"hello");

        assert_eq!(MessageContent::empty().as_text().unwrap(), "");

        let bare_binary = MessageContent::binary(vec![9]);
        assert!(bare_binary.headers().is_empty());
        assert_eq!(bare_binary.as_binary().unwrap(), vec![9]);

        let binary = MessageContent::binary_with_headers(headers(), vec![0, 1, 2]);
        assert_eq!(binary.format(), ContentFormat::Binary);
        assert_eq!(binary.as_binary().unwrap(), vec![0, 1, 2]);
        assert_eq!(binary.as_text().unwrap(), "\0\u{1}\u{2}");

        let object = MessageContent::object(&vec![1, 2, 3]).unwrap();
        assert_eq!(object.format(), ContentFormat::Object);
        assert_eq!(object.as_list::<i32>().unwrap(), vec![1, 2, 3]);

        assert!(MessageContent::new(headers(), ContentFormat::Text, Value::Bytes(vec![1])).is_err());
        assert!(MessageContent::new(headers(), ContentFormat::Binary, Value::Text("x".into())).is_err());
    }

    #[test]
    fn serialization_round_trips_and_validates_the_body() {
        let content = MessageContent::binary_with_headers(headers(), vec![7, 8]);
        let encoded = content.serialize().unwrap();
        let decoded = MessageContent::parse(&encoded).unwrap();
        assert_eq!(decoded.format(), ContentFormat::Binary);
        assert_eq!(decoded.headers(), content.headers());
        assert_eq!(decoded.body(), content.body());

        let invalid = serde_cbor::to_vec(&serde_cbor::Value::Map(
            [
                (
                    Value::Text("f".into()),
                    Value::Text("t".into()),
                ),
                (Value::Text("b".into()), Value::Bytes(vec![1])),
            ]
            .into_iter()
            .collect(),
        ))
        .unwrap();
        assert!(MessageContent::parse(&invalid).is_err());
    }

    #[test]
    fn object_accessors_require_object_content() {
        let object = MessageContent::object(&HashMap::from([("key", 7)])).unwrap();
        assert_eq!(
            object.as_map::<String, i32>().unwrap(),
            HashMap::from([("key".to_string(), 7)])
        );

        let text = MessageContent::text("value");
        assert!(text.as_object::<String>().is_err());
        assert!(text.as_list::<i32>().is_err());
        assert!(text.as_map::<String, i32>().is_err());
    }
}
