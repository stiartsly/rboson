use crate::messaging::internal::handshake::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn java_wire_roundtrip() {
        for body in [
            HandshakeType::FriendRequest(Some("hello".into())),
            HandshakeType::FriendRequest(Some(String::new())),
            HandshakeType::FriendRequest(None),
            HandshakeType::FriendRequestAccept(vec![7; crate::signature::PrivateKey::BYTES]),
        ] {
            let mut handshake = Handshake::new(42, body).unwrap();
            handshake.from = Some(Id::random());
            let bytes = serde_cbor::to_vec(&handshake).unwrap();
            let Value::Map(wire) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
                panic!("Handshake must serialize as a map");
            };
            assert_eq!(wire.len(), 3);
            assert_eq!(wire.get(&Value::Text("t".into())), Some(&Value::Integer(42)));
            let (tag, body) = match &handshake.body {
                HandshakeType::FriendRequest(hello) =>
                    ("fr", hello.clone().map(Value::Text).unwrap_or(Value::Null)),
                HandshakeType::FriendRequestAccept(key) => ("fra", Value::Bytes(key.clone())),
            };
            assert_eq!(wire.get(&Value::Text("y".into())), Some(&Value::Text(tag.into())));
            assert_eq!(wire.get(&Value::Text("b".into())), Some(&body));
            let decoded: Handshake = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded.timestamp, handshake.timestamp);
            assert_eq!(decoded.body, handshake.body);
            assert!(decoded.from.is_none());
        }
    }

    #[test]
    fn accepts_missing_greeting() {
        let wire = Value::Map([
            (Value::Text("t".into()), Value::Integer(42)),
            (Value::Text("y".into()), Value::Text("fr".into())),
        ].into_iter().collect());
        let handshake: Handshake = serde_cbor::from_slice(&serde_cbor::to_vec(&wire).unwrap()).unwrap();
        assert_eq!(handshake.body, HandshakeType::FriendRequest(None));
    }

    #[test]
    fn rejects_invalid_payload() {
        for (timestamp, tag, body) in [
            (-1, "fr", Some(Value::Text("hello".into()))),
            (42, "unknown", None),
            (42, "fr", Some(Value::Integer(1))),
            (42, "fra", None),
            (42, "fra", Some(Value::Null)),
            (42, "fra", Some(Value::Text("key".into()))),
            (42, "fra", Some(Value::Bytes(vec![0; 1]))),
        ] {
            let mut wire = [
                (Value::Text("t".into()), Value::Integer(timestamp)),
                (Value::Text("y".into()), Value::Text(tag.into())),
            ].into_iter().collect::<std::collections::BTreeMap<_, _>>();
            if let Some(body) = body {
                wire.insert(Value::Text("b".into()), body);
            }
            assert!(serde_cbor::from_slice::<Handshake>(&serde_cbor::to_vec(&Value::Map(wire)).unwrap()).is_err());
        }
        assert!(Handshake::new(-1, HandshakeType::FriendRequest(None)).is_err());
        assert!(Handshake::new(42, HandshakeType::FriendRequestAccept(Vec::new())).is_err());
    }
}
