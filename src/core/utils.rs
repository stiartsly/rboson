use std::result::Result as SResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use crate::{Id, core::cryptobox::Nonce};

pub(crate) fn serialize_id<S>(id: &Id, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&id.to_base58())
    } else {
        id.serialize(se)
    }
}

pub(crate) fn serialize_id_opt<S>(id: &Option<Id>, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    match id {
        Some(v) => serialize_id(v, se),
        _ => se.serialize_none(),
    }
}

pub(crate) fn deserialize_id<'de, D>(de: D) -> SResult<Id, D::Error>
where
    D: Deserializer<'de>,
{
    Id::deserialize(de)
}

pub(crate) fn deserialize_id_opt<'de, D>(de: D) -> SResult<Option<Id>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Id>::deserialize(de)
}

pub(crate) fn serialize_sig<S>(sig: &Vec<u8>, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    serialize_bytes(sig, se)
}

pub(crate) fn serialize_sig_opt<S>(sig: &Option<Vec<u8>>, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    serialize_bytes_opt(sig, se)
}

pub(crate) fn deserialize_sig<'de, D>(de: D) -> SResult<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bytes(de)
}

pub(crate) fn deserialize_sig_opt<'de, D>(de: D) -> SResult<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<u8>>::deserialize(de)
}

pub(crate) fn serialize_nonce_opt<S>(nonce: &Option<Nonce>, se: S) -> SResult<S::Ok, S::Error>
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

pub(crate) fn deserialize_nonce_opt<'de, D>(de: D) -> SResult<Option<Nonce>, D::Error>
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

pub(crate) fn serialize_bytes<S>(data: &Vec<u8>, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    if se.is_human_readable() {
        se.serialize_str(&format!("0x{}", hex::encode(data)))
    } else {
        data.serialize(se)
    }
}

pub(crate) fn serialize_bytes_opt<S>(data: &Option<Vec<u8>>, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    match data {
        Some(v) => serialize_bytes(v, se),
        _ => se.serialize_none(),
    }
}

pub(crate) fn deserialize_bytes<'de, D>(de: D) -> SResult<Vec<u8>, D::Error>
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

pub(crate) fn deserialize_bytes_opt<'de, D>(de: D) -> SResult<Option<Vec<u8>>, D::Error>
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

pub(crate) fn serialize_seq<S>(seq: &i32, se: S) -> SResult<S::Ok, S::Error>
where
    S: Serializer,
{
    seq.serialize(se)
}

pub(crate) fn deserialize_seq<'de, D>(de: D) -> SResult<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let seq = i32::deserialize(de)?;
    if seq < 0 {
        return Err(serde::de::Error::custom("seq must be larger than or equal to 0"));
    }
    Ok(seq)
}

pub(crate) const fn default_seq() -> i32 {
    0
}

pub(crate) fn is_default_seq(seq: &i32) -> bool {
    *seq == 0
}

pub(crate) fn is_default<T>(v: &T) -> bool
where
    T: Default + PartialEq,
{
    *v == T::default()
}
