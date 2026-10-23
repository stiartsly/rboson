use boson_api::{Id};
use crate::channel::Role;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelMember {
    #[serde(rename = "id")]
    id: Id,
    #[serde(rename = "r")]
    role: Role,
    #[serde(rename = "j")]
    joined: u64,
}

/// A single member of a channel, combining identity and role.
impl ChannelMember {
    pub(crate) fn new(id: Id, role: Role, joined: u64) -> Self {
        Self { id, role, joined }
    }

    pub(crate) fn id(&self) -> &Id {
        &self.id
    }

    pub(crate) fn role(&self) -> Role {
        self.role
    }

    pub(crate) fn joined(&self) -> u64 {
        self.joined
    }

    pub(crate) fn is_owner(&self) -> bool {
        self.role == Role::Owner
    }

    pub(crate) fn is_moderator(&self) -> bool {
        self.role == Role::Moderator
    }

    pub(crate) fn is_member(&self) -> bool {
        self.role == Role::Member
    }

    pub(crate) fn is_banned(&self) -> bool {
        self.role == Role::Banned
    }
}

#[cfg(test)]
#[path = "unitests/test_channel_member.rs"]
mod test_channel_member;
