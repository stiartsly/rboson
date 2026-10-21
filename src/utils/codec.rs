use std::{
    fmt,
    result::Result as StdResult,
};
use serde::{
    de::{self, Error, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use crate::{
    core::cryptobox::Nonce,
    Id,
    dht::version,
};

pub(crate) mod id {
    use super::*;

    pub(crate) fn serialize<S>(value: &Id, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if se.is_human_readable() {
            se.serialize_str(&value.to_base58())
        } else {
            value.serialize(se)
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Id, D::Error>
    where
        D: Deserializer<'de>,
    {
        Id::deserialize(de)
    }
}

pub(crate) mod id_opt {
    use super::*;

    pub(crate) fn serialize<S>(value: &Option<Id>, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => id::serialize(v, se),
            _ => se.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Option<Id>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<Id>::deserialize(de)
    }
}

pub(crate) mod nonce_opt {
    use super::*;

    pub(crate) fn serialize<S>(nonce: &Option<Nonce>, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match nonce {
            Some(v) => {
                if se.is_human_readable() {
                    se.serialize_str(&format!("0x{}", hex::encode(v.as_ref())))
                } else {
                    v.as_ref().serialize(se)
                }
            }
            _ => se.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Option<Nonce>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<Vec<u8>> = if de.is_human_readable() {
            let s_opt = Option::<String>::deserialize(de)?;
            s_opt.map(|s| {
                if s.starts_with("0x") {
                    hex::decode(&s[2..])
                } else {
                    hex::decode(&s)
                }
                .map_err(Error::custom)
            })
            .transpose()?
        } else {
            Option::<Vec<u8>>::deserialize(de)?
        };

        match opt {
            Some(raw) => {
                let nonce = Nonce::try_from(raw.as_slice())
                    .map_err(|e| Error::custom(format!("invalid nonce: {}", e)))?;
                Ok(Some(nonce))
            }
            _ => Ok(None),
        }
    }
}

pub(crate) mod bytes {
    use super::*;

    pub(crate) fn serialize<S>(value: &Vec<u8>, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if se.is_human_readable() {
            se.serialize_str(&format!("0x{}", hex::encode(value)))
        } else {
            se.serialize_bytes(value)
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if de.is_human_readable() {
            let s = String::deserialize(de)?;
            if s.starts_with("0x") {
                let hex_str = &s[2..];
                let bytes = hex::decode(hex_str).map_err(Error::custom)?;
                Ok(bytes)
            } else {
                Err(Error::custom("invalid hex string"))
            }
        } else {
            de.deserialize_any(ByteVecVisitor)
        }
    }
}

pub(crate) mod bytes_opt {
    use super::*;

    pub(crate) fn serialize<S>(value: &Option<Vec<u8>>, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => bytes::serialize(value, se),
            None => se.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if de.is_human_readable() {
            Option::<String>::deserialize(de)?
                .map(|s| {
                    let hex_str = s
                        .strip_prefix("0x")
                        .ok_or_else(|| Error::custom("invalid hex string"))?;
                    hex::decode(hex_str).map_err(Error::custom)
                })
                .transpose()
        } else {
            de.deserialize_option(OptionalByteVecVisitor)
        }
    }
}

struct ByteVecVisitor;
impl<'de> Visitor<'de> for ByteVecVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a CBOR byte string or sequence of bytes")
    }

    fn visit_bytes<E>(self, value: &[u8]) -> StdResult<Self::Value, E>
    where
        E: Error,
    {
        Ok(value.to_vec())
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> StdResult<Self::Value, E>
    where
        E: Error,
    {
        Ok(value)
    }

    fn visit_seq<A>(self, sequence: A) -> StdResult<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Vec::<u8>::deserialize(de::value::SeqAccessDeserializer::new(sequence))
    }
}

struct OptionalByteVecVisitor;
impl<'de> Visitor<'de> for OptionalByteVecVisitor {
    type Value = Option<Vec<u8>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an optional CBOR byte string or sequence of bytes")
    }

    fn visit_none<E>(self) -> StdResult<Self::Value, E>
    where
        E: Error,
    {
        Ok(None)
    }

    fn visit_some<D>(self, de: D) -> StdResult<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        bytes::deserialize(de).map(Some)
    }
}

pub(crate) mod seq {
    use super::*;

    pub(crate) fn serialize<S>(value: &i32, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if se.is_human_readable() {
            se.serialize_str(&value.to_string())
        } else if *value == 0 {
            se.serialize_none()
        } else {
            value.serialize(se)
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<i32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seq = Option::<i32>::deserialize(de)?.unwrap_or(0);
        if seq < 0 {
            return Err(Error::custom("seq must be larger than or equal to 0"));
        }
        Ok(seq)
    }
}

pub(crate) mod expected_seq {
    use super::*;

    pub(crate) fn serialize<S>(value: &i32, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if se.is_human_readable() {
            se.serialize_str(&value.to_string())
        } else if *value == -1 {
            se.serialize_none()
        } else {
            value.serialize(se)
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<i32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seq = Option::<i32>::deserialize(de)?.unwrap_or(-1);
        if seq < -1 {
            return Err(Error::custom("seq must be larger than or equal to -1"));
        }
        Ok(seq)
    }
}

pub(crate) mod ver {
    use super::*;

    pub(crate) fn serialize<S>(value: &i32, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if se.is_human_readable() {
            se.serialize_str(&format!("{}", version::format_version(*value)))
        } else {
            value.serialize(se)
        }
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<i32, D::Error>
    where
        D: Deserializer<'de>,
    {
        unimplemented!()
    }
}

pub(crate) fn is_default<T: IsDefault>(v: &T) -> bool {
    v.is_default()
}

pub(crate) trait IsDefault {
    fn is_default(&self) -> bool;
}

impl IsDefault for String {
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

impl<T> IsDefault for Vec<T> {
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

impl IsDefault for Option<String> {
    fn is_default(&self) -> bool {
        self.is_none() || self.as_ref().map_or(true, |v| v.is_empty())
    }
}

impl<T> IsDefault for Option<Vec<T>> {
    fn is_default(&self) -> bool {
        self.is_none() || self.as_ref().map_or(true, |v| v.is_empty())
    }
}

impl IsDefault for i32 {
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl IsDefault for u64 {
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl IsDefault for f64 {
    fn is_default(&self) -> bool {
        *self == 0.0
    }
}

impl IsDefault for bool {
    fn is_default(&self) -> bool {
        !*self
    }
}
