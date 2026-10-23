use std::fmt;
use serde::Serialize;

use crate::internal::codec;
use crate::Permission;
use boson_api::{utils, Id};

/// Channel creation request sent to the service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct NewChannelInfo {
    #[serde(rename = "sid", serialize_with = "utils::codec::id::serialize")]
    session_id: Id,

    #[serde(rename = "sk", serialize_with = "utils::codec::bytes::serialize")]
    session_key: Vec<u8>,

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
        skip_serializing_if = "Option::is_none"
    )]
    notice: Option<String>,

    #[serde(rename = "a", skip_serializing_if = "utils::is_default")]
    announce: bool,
}

impl NewChannelInfo {
    pub(crate) fn new(
        session_id: Id,
        session_key: Vec<u8>,
        permission: Permission,
        name: Option<String>,
        notice: Option<String>,
        announce: bool,
    ) -> Self {
        Self {
            session_id,
            session_key,
            permission,
            name,
            notice,
            announce,
        }
    }
}

impl fmt::Display for NewChannelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewChannelInfo")
            .field("session_id", &self.session_id)
            .field("session_key_len", &self.session_key.len())
            .field("permission", &self.permission)
            .field("name", &self.name)
            .field("notice", &self.notice)
            .field("announce", &self.announce)
            .finish()
    }
}
