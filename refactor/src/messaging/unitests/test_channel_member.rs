#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn roles_roundtrip() {
        for role in [Role::Owner, Role::Moderator, Role::Member, Role::Banned] {
            let member = ChannelMember::new(Id::random(), role, 12);
            let bytes = serde_cbor::to_vec(&member).unwrap();
            let Value::Map(fields) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
                panic!("Channel member must serialize as a map");
            };
            assert_eq!(fields.get(&Value::Text("r".into())), Some(&Value::Text(role.to_string())));
            let decoded: ChannelMember = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded, member);
            assert_eq!(decoded.id(), member.id());
            assert_eq!(decoded.role(), role);
            assert_eq!(decoded.joined(), 12);
            assert_eq!(decoded.is_owner(), role == Role::Owner);
            assert_eq!(decoded.is_moderator(), role == Role::Moderator);
            assert_eq!(decoded.is_member(), role == Role::Member);
            assert_eq!(decoded.is_banned(), role == Role::Banned);
        }
    }

    #[test]
    fn invalid_values() {
        for (role, joined) in [("Unknown", 12), ("Owner", -1)] {
            let wire = Value::Map([
                (Value::Text("id".into()), Value::Bytes(Id::default().as_bytes().to_vec())),
                (Value::Text("r".into()), Value::Text(role.into())),
                (Value::Text("j".into()), Value::Integer(joined.into())),
            ].into_iter().collect());
            assert!(serde_cbor::from_slice::<ChannelMember>(&serde_cbor::to_vec(&wire).unwrap()).is_err());
        }
    }
}
use super::*;
