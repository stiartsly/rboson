use serde::{Deserialize, Serialize};
use crate::Id;
use crate::messaging::channel;

#[derive(Serialize, Deserialize)]
pub(crate) struct UserProfile {
    #[serde(rename = "n")]
    name: Option<String>,
}

#[allow(dead_code)]
impl UserProfile {
    pub fn new(name: Option<String>) -> Self {
        Self { name }
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }
}

#[allow(dead_code)]
pub struct ContactRemove {}

#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub(crate) struct ChannelCreate {
    #[serde(rename = "sid")]
    session_id: Id,
    #[serde(rename = "p")]
    permission: channel::Permission,
    #[serde(rename = "n", skip_serializing_if = "crate::is_none_or_empty")]
    name: Option<String>,
    #[serde(rename = "d", skip_serializing_if = "crate::is_none_or_empty")]
    notice: Option<String>,
}

#[allow(unused)]
impl ChannelCreate {
    pub(crate) fn new(session_id: Id,
        permission: channel::Permission,
        name: Option<String>,
        notice: Option<String>
        ) -> Self {
        Self {
            session_id,
            permission,
            name,
            notice,
        }
    }

    pub(crate) fn session_id(&self) -> &Id {
        &self.session_id
    }

    pub(crate) fn permission(&self) -> channel::Permission {
        self.permission
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub(crate) struct ChannelMemberRole {
    #[serde(rename = "id")]
    members: Vec<Id>,
    #[serde(rename = "r")]
    role: channel::Role,
}

#[allow(unused)]
impl ChannelMemberRole {
    pub(crate) fn new(members: Vec<Id>, role: channel::Role) -> Self {
        Self { members, role }
    }

    pub(crate) fn members(&self) -> Vec<&Id> {
        self.members.iter()
            .map(|id| id)
            .collect::<Vec<&Id>>()
    }

    pub(crate) fn role(&self) -> channel::Role {
        self.role
    }
}
