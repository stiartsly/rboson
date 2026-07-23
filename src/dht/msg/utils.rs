
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

pub(crate) fn serialize_id<S>(target: &Id, se: S) -> SResult<S::Ok, S::Error>
where S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&target.to_base58())
    } else {
        target.serialize(se)
    }
}

pub(crate) fn deserialize_id<'de, D>(de: D) -> SResult<Id, D::Error>
where D: Deserializer<'de>,
{
    Id::deserialize(de)
}

pub(crate) fn is_default_seq(v: &i32) -> bool {
    *v < 0
}

pub(crate) fn default_seq() -> i32 { -1 }

pub(crate) fn deserialize_seq<'de, D>(de: D) -> SResult<i32, D::Error>
where  D: Deserializer<'de>,
{
    let seq = i32::deserialize(de)?;
    if seq < -1 {
        return Err(serde::de::Error::custom("expected_seq must be larger than or equal to -1"));
    }
    Ok(seq)
}

pub(crate) fn deserialize_count<'de, D>(de: D) -> SResult<i32, D::Error>
where D: Deserializer<'de>,
{
    let count = i32::deserialize(de)?;
    if count <= 0 {
        return Err(serde::de::Error::custom("expected_count must be at least larger than 0"));
    }
    Ok(count)
}
