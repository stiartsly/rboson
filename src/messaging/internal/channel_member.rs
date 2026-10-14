use crate::{Id, messaging::channel::Role};
use serde::{de::Error as DeError, Serialize, Deserialize, Deserializer, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ChannelMember {
    id: Id,
    #[serde(rename = "r", with = "role")]
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
        matches!(self.role, Role::Owner)
    }

    pub(crate) fn is_moderator(&self) -> bool {
        matches!(self.role, Role::Moderator)
    }

    pub(crate) fn is_member(&self) -> bool {
        matches!(self.role, Role::Member)
    }

    pub(crate) fn is_banned(&self) -> bool {
        self.role.is_banned()
    }
}

mod role {
    use super::*;

    pub(super) fn serialize<S>(value: &Role, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(*value as i32)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Role, D::Error>
    where
        D: Deserializer<'de>,
    {
        Role::try_from(i32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn channel_member_round_trips_numeric_roles() {
        for role in [Role::Owner, Role::Moderator, Role::Member, Role::Banned] {
            let member = ChannelMember::new(Id::random(), role, 12);
            let bytes = serde_cbor::to_vec(&member).unwrap();
            let Value::Map(fields) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
                panic!("Channel member must serialize as a map");
            };
            assert_eq!(fields.get(&Value::Text("r".into())), Some(&Value::Integer((role as i32).into())));
            let decoded: ChannelMember = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded, member);
            assert_eq!(decoded.id(), member.id());
            assert_eq!(decoded.role(), role);
            assert_eq!(decoded.joined(), 12);
        }
    }

    #[test]
    fn channel_member_rejects_invalid_roles_and_timestamps() {
        for (role, joined) in [(3, 12), (-2, 12), (0, -1)] {
            let wire = Value::Map([
                (Value::Text("id".into()), Value::Bytes(Id::default().as_bytes().to_vec())),
                (Value::Text("r".into()), Value::Integer(role.into())),
                (Value::Text("j".into()), Value::Integer(joined.into())),
            ].into_iter().collect());
            assert!(serde_cbor::from_slice::<ChannelMember>(&serde_cbor::to_vec(&wire).unwrap()).is_err());
        }
    }
}