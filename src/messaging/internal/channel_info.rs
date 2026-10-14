use std::time::{Duration, SystemTime};
use serde::{
    de::Error as DeError,
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{Id, utils};
use crate::messaging::{
    channel::Permission,
};
use super::ChannelMember;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ChannelInfo {
    #[serde(rename = "id")]
    channel_id: Id,

    #[serde(rename = "o")]
    owner_id: Id,

    #[serde(rename = "sid")]
    session_id: Id,

    #[serde(
        rename = "sk",
        default,
        serialize_with = "serialize_session_key",
        deserialize_with = "utils::deserialize_bytes_opt",
        skip_serializing_if = "is_bytes_none_or_empty"
    )]
    session_key: Option<Vec<u8>>,

    #[serde(rename = "p", with = "permission")]
    permission: Permission,

    #[serde(
        rename = "n",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    name: Option<String>,

    #[serde(
        rename = "nt",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    notice: Option<String>,

    #[serde(
        rename = "a",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    announce: bool,

    #[serde(rename = "c", default)]
    created_at: u64,

    #[serde(rename = "u", default)]
    update_at: u64,

    #[serde(
        rename = "m",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    members: Vec<ChannelMember>,
}

impl ChannelInfo {
    pub(crate) fn new(
        channel_id: Id,
        owner_id: Id,
        session_id: Id,
        session_key: Option<Vec<u8>>,
        permission: Permission,
        name: Option<String>,
        notice: Option<String>,
        announce: bool,
        created_at: u64,
        update_at: u64,
        members: Vec<ChannelMember>,
    ) -> Self {
        Self {
            channel_id,
            owner_id,
            session_id,
            session_key,
            permission,
            name,
            notice,
            announce,
            created_at,
            update_at,
            members,
        }
    }

    pub(crate) fn channel_id(&self) -> &Id {
        &self.channel_id
    }

    pub(crate) fn owner_id(&self) -> &Id {
        &self.owner_id
    }

    pub(crate) fn session_id(&self) -> &Id {
        &self.session_id
    }

    pub(crate) fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    pub(crate) fn permission(&self) -> Permission {
        self.permission
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub(crate) fn announce(&self) -> bool {
        self.announce
    }

    pub(crate) fn created_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.created_at)
    }

    pub(crate) fn update_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.update_at)
    }

    pub(crate) fn members(&self) -> &[ChannelMember] {
        &self.members
    }
}

fn is_none_or_empty(value: &Option<String>) -> bool {
    value.as_ref().map_or(true, String::is_empty)
}

fn is_bytes_none_or_empty(value: &Option<Vec<u8>>) -> bool {
    value.as_ref().map_or(true, Vec::is_empty)
}

fn serialize_session_key<S>(value: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) if serializer.is_human_readable() => utils::serialize_bytes(value, serializer),
        Some(value) => serializer.serialize_bytes(value),
        None => serializer.serialize_none(),
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

mod permission {
    use super::*;

    pub(super) fn serialize<S>(value: &Permission, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(*value as i32)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Permission, D::Error>
    where
        D: Deserializer<'de>,
    {
        Permission::try_from(i32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[cfg(test)]
#[path = "../unitests/test_channel_info.rs"]
mod test_channel_info;
