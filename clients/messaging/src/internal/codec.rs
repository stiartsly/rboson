use serde::{
    de::Error as DeError,
    Deserialize,
    Deserializer,
    Serializer
};
use crate::channel::{Permission, Role};

pub(crate) mod role {
    use super::*;

    pub(crate) fn serialize<S>(value: &Role, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        se.serialize_i32(i32::from(*value))
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Role, D::Error>
    where
        D: Deserializer<'de>,
    {
        Role::try_from(i32::deserialize(de)?).map_err(D::Error::custom)
    }
}

pub(crate) mod permission {
    use super::*;

    pub(crate) fn serialize<S>(value: &Permission, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        se.serialize_i32(i32::from(*value))
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Permission, D::Error>
    where
        D: Deserializer<'de>,
    {
        Permission::try_from(i32::deserialize(de)?).map_err(D::Error::custom)
    }
}
