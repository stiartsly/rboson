//! `Id` serde adapter for W3C DID documents.
//!
//! Text formats use the `did:boson:<id>` form, matching the Java W3C DID
//! context; CBOR keeps the raw identifier bytes.

use serde::{Deserialize, Deserializer, Serializer};

use crate::Id;

use super::DIDUrl;

pub(crate) fn serialize<S>(value: &Id, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(&value.to_did_string())
    } else {
        serializer.serialize_bytes(value.as_bytes())
    }
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Id, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let value = String::deserialize(deserializer)?;
        // Accept both a DID URL and a bare base58 identifier.
        if value.starts_with("did:") {
            DIDUrl::parse(&value)
                .map_err(serde::de::Error::custom)?
                .id()
                .copied()
                .ok_or_else(|| serde::de::Error::custom("DID URL has no identifier"))
        } else {
            Id::try_from(value.as_str()).map_err(serde::de::Error::custom)
        }
    } else {
        Id::deserialize(deserializer)
    }
}

pub(crate) fn serialize_opt<S>(
    value: &Option<Id>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serialize(value, serializer),
        None => serializer.serialize_none(),
    }
}

pub(crate) fn deserialize_opt<'de, D>(deserializer: D) -> Result<Option<Id>, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                if value.starts_with("did:") {
                    DIDUrl::parse(&value)
                        .map_err(serde::de::Error::custom)?
                        .id()
                        .copied()
                        .ok_or_else(|| serde::de::Error::custom("DID URL has no identifier"))
                } else {
                    Id::try_from(value.as_str()).map_err(serde::de::Error::custom)
                }
            })
            .transpose()
    } else {
        Option::<Id>::deserialize(deserializer)
    }
}
