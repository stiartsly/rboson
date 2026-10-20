use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};

use crate::messaging::channel::Permission;
use crate::messaging::internal::codec;
use crate::{utils, Id};

use super::ChannelMember;

/// Channel metadata and membership as sent by the service.
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
        serialize_with = "codec::optional_bytes::serialize",
        deserialize_with = "codec::optional_bytes::deserialize",
        skip_serializing_if = "is_bytes_none_or_empty"
    )]
    session_key: Option<Vec<u8>>,

    #[serde(rename = "p", with = "codec::permission")]
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
        skip_serializing_if = "codec::is_none_or_empty"
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
    updated_at: u64,

    #[serde(
        rename = "m",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    members: Vec<ChannelMember>,
}

impl ChannelInfo {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
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
        updated_at: u64,
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
            updated_at,
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

    pub(crate) fn updated_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.updated_at)
    }

    pub(crate) fn members(&self) -> &[ChannelMember] {
        &self.members
    }
}

fn is_bytes_none_or_empty(value: &Option<Vec<u8>>) -> bool {
    value.as_ref().map_or(true, Vec::is_empty)
}

#[cfg(test)]
#[path = "../unitests/test_channel_info.rs"]
mod test_channel_info;
