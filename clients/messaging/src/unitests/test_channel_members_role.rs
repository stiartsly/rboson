use serde_cbor::Value;

use boson_api::Id;
use crate::Role;
use super::ChannelMembersRole;

#[test]
fn test_serde() {
    let members_role = ChannelMembersRole::new(
        vec![Id::default()],
        Role::Moderator
    );
    let value = serde_cbor::value::to_value(&members_role).unwrap();
    let Value::Map(fields) = &value else {
        panic!("channel member role must serialize as a map");
    };
    assert_eq!(
        fields.get(&Value::Text("r".to_string())),
        Some(&Value::Integer(1))
    );

    let encoded = serde_cbor::to_vec(&members_role).unwrap();
    let decoded = serde_cbor::from_slice::<ChannelMembersRole>(&encoded).unwrap();
    assert_eq!(decoded, members_role);
}
