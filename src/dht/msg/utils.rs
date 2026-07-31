
use std::result::Result as SResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use crate::{Id, core::version};

pub(crate) fn serialize_ver<S>(ver: &i32, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&format!("{}", version::format_version(*ver)))
    } else {
        ver.serialize(se)
    }
}

pub(crate) fn serialize_id<S>(id: &Id, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&id.to_base58())
    } else {
        id.serialize(se)
    }
}

pub(crate) fn serialize_id_opt<S>(id: &Option<Id>, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    match id {
        Some(v) => serialize_id(v, se),
        _ => se.serialize_none(),
    }
}

pub(crate) fn deserialize_id<'de, D>(de: D) -> SResult<Id, D::Error>
where D: Deserializer<'de>,
{
    Id::deserialize(de)
}

pub(crate) fn deserialize_id_opt<'de, D>(de: D) -> SResult<Option<Id>, D::Error>
where D: Deserializer<'de>,
{
    Option::<Id>::deserialize(de)
}

pub(crate) fn serialize_nonce_opt<S>(nonce: &Option<Vec<u8>>, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    serialize_bytes_opt(nonce, se)
}

pub(crate) fn deserialize_nonce_opt<'de, D>(de: D) -> SResult<Option<Vec<u8>>, D::Error>
where D: Deserializer<'de>,
{
    Option::<Vec<u8>>::deserialize(de)
}

pub(crate) fn serialize_sig<S>(sig: &Vec<u8>, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    serialize_bytes(sig, se)
}

pub(crate) fn serialize_sig_opt<S>(sig: &Option<Vec<u8>>, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    serialize_bytes_opt(sig, se)
}

pub(crate) fn deserialize_sig<'de, D>(de: D) -> SResult<Vec<u8>, D::Error>
where D: Deserializer<'de>,
{
    Vec::<u8>::deserialize(de)
}

pub(crate) fn deserialize_sig_opt<'de, D>(de: D) -> SResult<Option<Vec<u8>>, D::Error>
where D: Deserializer<'de>,
{
    Option::<Vec<u8>>::deserialize(de)
}

pub(crate) fn serialize_bytes<S>(data: &Vec<u8>, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        let data = format!("0x{}", hex::encode(data));
        se.serialize_str(&data)
    } else {
        data.serialize(se)
    }
}

pub(crate) fn serialize_bytes_opt<S>(data: &Option<Vec<u8>>, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    match data {
        Some(v) => serialize_bytes(v, se),
        _ => se.serialize_none(),
    }
}

pub(crate) fn deserialize_bytes<'de, D>(de: D) -> SResult<Vec<u8>, D::Error>
where D: Deserializer<'de>,
{
    Vec::<u8>::deserialize(de)
}

pub(crate) fn deserialize_bytes_opt<'de, D>(de: D) -> SResult<Option<Vec<u8>>, D::Error>
where D: Deserializer<'de>,
{
    Option::<Vec<u8>>::deserialize(de)
}

pub(crate) fn serialize_seq<S>(seq: &i32, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&format!("{}", seq))
    } else {
        if *seq == -1 {
            se.serialize_none()
        } else {
            seq.serialize(se)
        }
    }
}

pub(crate) fn deserialize_seq<'de, D>(de: D) -> SResult<i32, D::Error>
where  D: Deserializer<'de>,
{
    let seq = Option::<i32>::deserialize(de)?.unwrap_or(-1);
    if seq < -1 {
        return Err(serde::de::Error::custom("expected_seq must be larger than or equal to -1"));
    }
    Ok(seq)
}

pub(crate) fn serialize_count<S>(count: &i32, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&format!("{}", count))
    } else {
        if *count == 0 {
            se.serialize_none()
        } else {
            count.serialize(se)
        }
    }
}

pub(crate) const fn default_seq() -> i32 { -1 }

pub(crate) fn deserialize_count<'de, D>(de: D) -> SResult<i32, D::Error>
where  D: Deserializer<'de>,
{
    let count = i32::deserialize(de)?;
    if count < 0 {
        return Err(serde::de::Error::custom("expected_count must be larger than or equal to -1"));
    }
    Ok(count)
}

pub(crate) fn is_default_seq(seq: &i32) -> bool {
    *seq == -1
}
