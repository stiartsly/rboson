use serde::{
    de::Error as DeError,
    Deserialize,
    Deserializer,
    Serializer
};
use crate::messaging::channel::{Permission, Role};

/// Serializes [`Role`] using its numeric wire representation.
pub(crate) mod role {
    use super::*;

    pub(crate) fn serialize<S>(value: &Role, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(i32::from(*value))
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Role, D::Error>
    where
        D: Deserializer<'de>,
    {
        Role::try_from(i32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Serializes [`Permission`] using its numeric wire representation.
pub(crate) mod permission {
    use super::*;

    pub(crate) fn serialize<S>(value: &Permission, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(i32::from(*value))
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Permission, D::Error>
    where
        D: Deserializer<'de>,
    {
        Permission::try_from(i32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Serializes a required byte string as hex text or CBOR bytes.
pub(crate) mod bytes {
    use super::*;

    pub(crate) fn serialize<S>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            crate::utils::serialize_bytes(value, serializer)
        } else {
            serializer.serialize_bytes(value)
        }
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        crate::utils::deserialize_bytes(deserializer)
    }
}

/// Serializes an optional byte string, omitting `None` at the field level.
pub(crate) mod optional_bytes {
    use super::*;

    pub(crate) fn serialize<S>(value: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => bytes::serialize(value, serializer),
            _ => serializer.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        crate::utils::deserialize_bytes_opt(deserializer)
    }
}

/// Returns `true` when a string field is absent or empty.
pub(crate) fn is_none_or_empty(value: &Option<String>) -> bool {
    value.as_deref().map_or(true, str::is_empty)
}
