use serde::{Deserialize, Serialize};

use crate::channel::Role;
use crate::internal::codec;
use boson_api::Id;

/// Role change request for one or more channel members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelMembersRole {
    #[serde(rename = "ids")]
    member_ids: Vec<Id>,
    #[serde(rename = "r", with = "codec::role")]
    role: Role,
}

impl ChannelMembersRole {
    #[cfg(test)]
    pub(crate) fn new(member_ids: Vec<Id>, role: Role) -> Self {
        Self { member_ids, role }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn serde_channel_members_role() {
        let dto = ChannelMembersRole::new(vec![Id::default()], Role::Moderator);
        let value = serde_cbor::value::to_value(&dto).unwrap();
        let Value::Map(fields) = &value else {
            panic!("channel member role must serialize as a map");
        };

        assert_eq!(
            fields.get(&Value::Text("r".to_string())),
            Some(&Value::Integer(1))
        );
        let decoded: ChannelMembersRole =
            serde_cbor::from_slice(&serde_cbor::to_vec(&dto).unwrap()).unwrap();
        assert_eq!(decoded, dto);
    }
}
