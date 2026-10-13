use serde::{
    de::Error as DeError,
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{Id, messaging::channel::Role};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelMembersRole {
    #[serde(rename = "ids")]
    member_ids: Vec<Id>,
    #[serde(rename = "r", with = "role")]
    role: Role,
}

impl ChannelMembersRole {
    pub(crate) fn new(member_ids: Vec<Id>, role: Role) -> Self {
        Self { member_ids, role }
    }

    pub(crate) fn member_ids(&self) -> &[Id] {
        &self.member_ids
    }

    pub(crate) fn role(&self) -> Role {
        self.role
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
    fn channel_members_role_round_trips_numeric_role() {
        let dto = ChannelMembersRole::new(vec![Id::default()], Role::Moderator);
        let value = serde_cbor::value::to_value(&dto).unwrap();
        let Value::Map(fields) = &value else {
            panic!("channel member role must serialize as a map");
        };

        assert_eq!(fields.get(&Value::Text("r".to_string())), Some(&Value::Integer(1)));
        let decoded: ChannelMembersRole =
            serde_cbor::from_slice(&serde_cbor::to_vec(&dto).unwrap()).unwrap();
        assert_eq!(decoded, dto);
    }
}
