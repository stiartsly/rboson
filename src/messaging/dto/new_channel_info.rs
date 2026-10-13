use std::fmt;
use serde::Serialize;

use crate::{
    id::Id,
    messaging::Permission,
    utils,
};

#[derive(Serialize, Hash, Eq)]
pub(crate) struct NewChannelInfo {
    #[serde(
        rename = "sid",
        serialize_with = "utils::serialize_id"
    )]
    session_id: Id,
    #[serde(rename = "sk", serialize_with = "utils::serialize_bytes")]
    session_key: Vec<u8>,
    #[serde(rename = "p")]
    permission: i32,
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
    #[serde(
        rename = "a",
        skip_serializing_if = "utils::is_default"
    )]
    announce: bool,
}

impl NewChannelInfo {
    pub fn new(
        session_id: Id,
        session_key: Vec<u8>,
        permission: Permission,
        name: Option<String>,
        notice: Option<String>,
        announce: bool,
    ) -> Self {
        Self {
            session_id: session_id,
            session_key,
            permission: permission as i32,
            name,
            notice,
            announce,
        }
    }

    pub(crate) fn session_id(&self) -> &[u8] {
        self.session_id.as_bytes()
    }

    pub(crate) fn session_key(&self) -> &[u8] {
        &self.session_key
    }

    pub(crate) fn permission(&self) -> Permission {
        Permission::try_from(self.permission).unwrap()
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
}

impl PartialEq for NewChannelInfo {
    fn eq(&self, other: &Self) -> bool {
        self.session_id == other.session_id
            && self.session_key == other.session_key
            && self.permission == other.permission
            && self.name == other.name
            && self.notice == other.notice
            && self.announce == other.announce
    }
}

impl fmt::Display for NewChannelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NewChannelInfo {{ session_id: {:?}, session_key: {:?}, permission: {:?}, name: {:?}, notice: {:?}, announce: {} }}",
            self.session_id,
            self.session_key,
            self.permission,
            self.name,
            self.notice,
            self.announce
        )
    }
}
