//! Byte-array serde matching the Java DID wire format.
//!
//! Text formats use Base64 URL without padding, while CBOR keeps raw bytes.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serializer};
use std::fmt;

pub(crate) fn serialize<S>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(&URL_SAFE_NO_PAD.encode(value))
    } else {
        serializer.serialize_bytes(value)
    }
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let value = String::deserialize(deserializer)?;
        URL_SAFE_NO_PAD
            .decode(value.as_bytes())
            .map_err(D::Error::custom)
    } else {
        deserializer.deserialize_any(ByteVisitor)
    }
}

struct ByteVisitor;

impl<'de> Visitor<'de> for ByteVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a byte string or sequence of bytes")
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(value.to_vec())
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(value)
    }

    fn visit_seq<A>(self, sequence: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        Vec::<u8>::deserialize(serde::de::value::SeqAccessDeserializer::new(sequence))
    }
}

#[cfg(test)]
mod tests {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Bytes {
        #[serde(with = "super")]
        value: Vec<u8>,
    }

    #[test]
    fn uses_base64_url_for_json_and_bytes_for_cbor() {
        let value = Bytes {
            value: vec![0, 1, 2, 253, 254, 255],
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#"{"value":"AAEC_f7_"}"#);
        assert_eq!(serde_json::from_str::<Bytes>(&json).unwrap().value, value.value);

        let cbor = serde_cbor::to_vec(&value).unwrap();
        assert_eq!(serde_cbor::from_slice::<Bytes>(&cbor).unwrap().value, value.value);
    }
}
