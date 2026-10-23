use serde::{Deserialize, Serialize};

use crate::channel::Role;
use crate::internal::codec;
use boson_api::Id;

/// A single member of a channel, combining identity and role.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ChannelMember {
    id: Id,
    #[serde(rename = "r", with = "codec::role")]
    role: Role,
    #[serde(rename = "j")]
    joined: u64,
}

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

    pub(crate) fn is_banned(&self) -> bool {
        self.role.is_banned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn serde_channel_member() {
        for role in [Role::Owner, Role::Moderator, Role::Member, Role::Banned] {
            let member = ChannelMember::new(Id::random(), role, 12);
            let bytes = serde_cbor::to_vec(&member).unwrap();
            let Value::Map(fields) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
                panic!("Channel member must serialize as a map");
            };
            assert_eq!(
                fields.get(&Value::Text("r".into())),
                Some(&Value::Integer((role as i32).into()))
            );

            let decoded: ChannelMember = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded, member);
            assert_eq!(decoded.id(), member.id());
            assert_eq!(decoded.role(), role);
            assert_eq!(decoded.joined(), 12);
        }
    }
}
