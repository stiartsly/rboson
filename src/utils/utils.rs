use std::result::Result as StdResult;
use serde::{
    Deserialize, Deserializer,
    Serialize, Serializer
};
use crate::{
    core::version,
    Id,
    core::cryptobox::Nonce
};

pub(crate) fn serialize_id<S>(id: &Id, se: S) -> StdResult<S::Ok, S::Error>
where
    S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&id.to_base58())
    } else {
        id.serialize(se)
    }
}

pub(crate) fn serialize_id_opt<S>(id: &Option<Id>, se: S) -> StdResult<S::Ok, S::Error>
where
    S: Serializer,
{
    match id {
        Some(v) => serialize_id(v, se),
        _ => se.serialize_none(),
    }
}

pub(crate) fn deserialize_id<'de, D>(de: D) -> StdResult<Id, D::Error>
where
    D: Deserializer<'de>,
{
    Id::deserialize(de)
}

pub(crate) fn deserialize_id_opt<'de, D>(de: D) -> StdResult<Option<Id>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Id>::deserialize(de)
}

pub(crate) fn serialize_nonce_opt<S>(nonce: &Option<Nonce>, se: S) -> StdResult<S::Ok, S::Error>
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

pub(crate) fn deserialize_nonce_opt<'de, D>(de: D) -> StdResult<Option<Nonce>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<Vec<u8>> = if de.is_human_readable() {
        let s_opt = Option::<String>::deserialize(de)?;
        s_opt
            .map(|s| {
                if s.starts_with("0x") {
                    hex::decode(&s[2..])
                } else {
                    hex::decode(&s)
                }
                .map_err(serde::de::Error::custom)
            })
            .transpose()?
    } else {
        Option::<Vec<u8>>::deserialize(de)?
    };

    match opt {
        Some(raw) => {
            let nonce = Nonce::try_from(raw.as_slice())
                .map_err(|e| serde::de::Error::custom(format!("invalid nonce: {}", e)))?;
            Ok(Some(nonce))
        }
        _ => Ok(None),
    }
}

pub(crate) fn serialize_bytes<S>(data: &Vec<u8>, se: S) -> StdResult<S::Ok, S::Error>
where
    S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&format!("0x{}", hex::encode(data)))
    } else {
        data.serialize(se)
    }
}

pub(crate) fn serialize_bytes_opt<S>(data: &Option<Vec<u8>>, se: S) -> StdResult<S::Ok, S::Error>
where
    S: Serializer,
{
    match data {
        Some(v) => serialize_bytes(v, se),
        _ => se.serialize_none(),
    }
}

pub(crate) fn deserialize_bytes<'de, D>(de: D) -> StdResult<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    if de.is_human_readable() {
        let s = String::deserialize(de)?;
        if s.starts_with("0x") {
            let hex_str = &s[2..];
            let bytes = hex::decode(hex_str).map_err(serde::de::Error::custom)?;
            Ok(bytes)
        } else {
            Err(serde::de::Error::custom("invalid hex string"))
        }
    } else {
        Vec::<u8>::deserialize(de)
    }
}

pub(crate) fn deserialize_bytes_opt<'de, D>(de: D) -> StdResult<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    if de.is_human_readable() {
        let opt: Option<String> = Option::<String>::deserialize(de)?;
        match opt {
            Some(s) => {
                if s.starts_with("0x") {
                    let hex_str = &s[2..];
                    let bytes = hex::decode(hex_str).map_err(serde::de::Error::custom)?;
                    Ok(Some(bytes))
                } else {
                    Err(serde::de::Error::custom("invalid hex string"))
                }
            }
            _ => Ok(None),
        }
    } else {
        Option::<Vec<u8>>::deserialize(de)
    }
}

pub(crate) fn deserialize_seq<'de, D>(de: D) -> StdResult<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let seq = i32::deserialize(de)?;
    if seq < 0 {
        println!("######>>>> seq: {}", seq);
        return Err(serde::de::Error::custom("seq must be larger than or equal to 0"));
    }
    Ok(seq)
}

pub(crate) fn deserialize_expected_seq<'de, D>(de: D) -> StdResult<i32, D::Error>
where  D: Deserializer<'de>,
{
    let seq = Option::<i32>::deserialize(de)?.unwrap_or(-1);
    if seq < -1 {
        return Err(serde::de::Error::custom("expected_seq must be larger than or equal to -1"));
    }
    Ok(seq)
}

pub(crate) const fn default_expected_seq() -> i32 { -1 }

pub(crate) fn is_default_expected_seq(seq: &i32) -> bool {
    *seq == -1
}

pub(crate) fn deserialize_count<'de, D>(de: D) -> StdResult<i32, D::Error>
where  D: Deserializer<'de>,
{
    let count = i32::deserialize(de)?;
    if count < 0 {
        return Err(serde::de::Error::custom("count must be larger than or equal to -1"));
    }
    Ok(count)
}

pub(crate) fn serialize_ver<S>(ver: &i32, se: S) -> StdResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&format!("{}", version::format_version(*ver)))
    } else {
        ver.serialize(se)
    }
}

#[allow(unused)]
pub(crate) fn is_default<T: IsDefault>(v: &T) -> bool {
    v.is_default()
}

pub(crate) trait IsDefault {
    fn is_default(&self) -> bool;
}

impl<T> IsDefault for Option<T> {
    fn is_default(&self) -> bool {
        self.is_none()
    }
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
