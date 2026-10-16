use crate::Id;
use crate::messaging::{
    channel::Permission,
    internal::ChannelInfo,
};
use super::ChannelMember;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::channel::Role;
    use serde_cbor::Value;

    #[test]
    fn channel_info_round_trips_numeric_permission_and_role() {
        let info = ChannelInfo::new(
            Id::default(),
            Id::default(),
            Id::default(),
            Some(vec![1, 2, 3]),
            Permission::Public,
            Some("channel".to_string()),
            None,
            false,
            10,
            11,
            vec![ChannelMember::new(Id::default(), Role::Banned, 12)],
        );
        let value = serde_cbor::value::to_value(&info).unwrap();
        let fields = match &value {
            Value::Map(fields) => fields,
            _ => panic!("channel info must serialize as a map"),
        };

        assert_eq!(fields.get(&Value::Text("p".to_string())), Some(&Value::Integer(0)));
        assert_eq!(
            fields.get(&Value::Text("sk".to_string())),
            Some(&Value::Bytes(vec![1, 2, 3]))
        );
        let members = fields.get(&Value::Text("m".to_string())).unwrap();
        let Value::Array(members) = members else {
            panic!("channel members must serialize as an array");
        };
        let Value::Map(member) = &members[0] else {
            panic!("channel member must serialize as a map");
        };
        assert_eq!(member.get(&Value::Text("r".to_string())), Some(&Value::Integer(-1)));

        let decoded: ChannelInfo = serde_cbor::from_slice(&serde_cbor::to_vec(&info).unwrap()).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn channel_info_defaults_optional_fields_and_requires_identity_and_permission() {
        let required = [
            (Value::Text("id".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
            (Value::Text("o".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
            (Value::Text("sid".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
            (Value::Text("p".into()), Value::Integer(0)),
        ].into_iter().collect::<std::collections::BTreeMap<_, _>>();
        let info: ChannelInfo = serde_cbor::value::from_value(Value::Map(required.clone())).unwrap();
        assert_eq!(info.permission(), Permission::Public);
        assert_eq!(info.session_key(), None);
        assert_eq!(info.name(), None);
        assert_eq!(info.notice(), None);
        assert!(!info.announce());
        assert_eq!(info.created_at(), std::time::UNIX_EPOCH);
        assert_eq!(info.update_at(), std::time::UNIX_EPOCH);
        assert!(info.members().is_empty());

        for field in ["id", "o", "sid", "p"] {
            let mut wire = required.clone();
            wire.remove(&Value::Text(field.into()));
            assert!(serde_cbor::value::from_value::<ChannelInfo>(Value::Map(wire)).is_err(), "{field}");
        }
        let mut wire = required.clone();
        wire.insert(Value::Text("sk".into()), Value::Null);
        let info: ChannelInfo = serde_cbor::value::from_value(Value::Map(wire)).unwrap();
        assert_eq!(info.session_key(), None);
        for permission in [-1, 4] {
            let mut wire = required.clone();
            wire.insert(Value::Text("p".into()), Value::Integer(permission.into()));
            assert!(serde_cbor::value::from_value::<ChannelInfo>(Value::Map(wire)).is_err());
        }
    }

    #[test]
    fn channel_info_preserves_metadata_and_omits_empty_optional_fields() {
        let channel_id = Id::random();
        let owner_id = Id::random();
        let session_id = Id::random();
        let mut info = ChannelInfo::new(
            channel_id, owner_id, session_id, Some(vec![1, 2, 3]),
            Permission::OwnerInvite, Some("channel".into()), Some("notice".into()),
            true, 10, 11, vec![ChannelMember::new(owner_id, Role::Owner, 10)],
        );
        let decoded: ChannelInfo = serde_cbor::value::from_value(serde_cbor::value::to_value(&info).unwrap()).unwrap();
        assert_eq!(decoded.channel_id(), &channel_id);
        assert_eq!(decoded.owner_id(), &owner_id);
        assert_eq!(decoded.session_id(), &session_id);
        assert_eq!(decoded.session_key(), Some([1, 2, 3].as_slice()));
        assert_eq!(decoded.permission(), Permission::OwnerInvite);
        assert_eq!(decoded.name(), Some("channel"));
        assert_eq!(decoded.notice(), Some("notice"));
        assert!(decoded.announce());
        assert_eq!(decoded.created_at(), std::time::UNIX_EPOCH + std::time::Duration::from_millis(10));
        assert_eq!(decoded.update_at(), std::time::UNIX_EPOCH + std::time::Duration::from_millis(11));
        assert_eq!(decoded.members(), info.members());
        assert_eq!(decoded, info);
        use std::hash::{Hash, Hasher};
        let mut original_hash = std::collections::hash_map::DefaultHasher::new();
        let mut decoded_hash = std::collections::hash_map::DefaultHasher::new();
        info.hash(&mut original_hash);
        decoded.hash(&mut decoded_hash);
        assert_eq!(original_hash.finish(), decoded_hash.finish());

        info.session_key = Some(Vec::new());
        info.name = Some(String::new());
        info.notice = Some(String::new());
        info.announce = false;
        info.members.clear();
        let Value::Map(wire) = serde_cbor::value::to_value(&info).unwrap() else {
            panic!("Channel info must serialize as a map");
        };
        for field in ["sk", "nt", "a", "m"] {
            assert!(!wire.contains_key(&Value::Text(field.into())), "{field}");
        }
        assert_eq!(wire.get(&Value::Text("n".into())), Some(&Value::Text(String::new())));
    }
}