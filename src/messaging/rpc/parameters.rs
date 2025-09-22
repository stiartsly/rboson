use serde::{Deserialize, Serialize};
use crate::Id;
use crate::messaging::channel;

#[derive(Serialize, Deserialize)]
pub(crate) struct UserProfile {
    #[serde(rename = "n")]
    name: Option<String>,
}

#[allow(unused)]
impl UserProfile {
    pub(crate) fn new(name: Option<String>) -> Self {
        Self { name }
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ContactRemove {
    #[serde(rename = "s", skip_serializing_if = "crate::is_none_or_empty")]
    sequence_id: Option<String>,

    #[serde(rename = "c", skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<Id>,
}

#[allow(unused)]
impl ContactRemove {
    pub(crate) fn new(sequence_id: Option<String>, contacts: Option<Vec<Id>>) -> Self {
        Self {
            sequence_id,
            contacts: contacts.unwrap_or_default()
        }
    }

    pub(crate) fn sequence_id(&self) -> Option<&str> {
        self.sequence_id.as_deref()
    }

    pub(crate) fn contacts(&self) -> &[Id] {
        &self.contacts
    }
}

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
    pub(crate) fn new(
            session_id: Id,
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

    pub(crate) fn members(&self) -> &[Id] {
        &self.members
    }

    pub(crate) fn role(&self) -> channel::Role {
        self.role
    }
}
