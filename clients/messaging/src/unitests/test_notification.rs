use serde_cbor::Value;

use boson_api::Id;
use crate::internal::notification::Notification;

fn wire(event: &str, body: Option<Value>) -> Vec<u8> {
    let mut map = [
        (Value::Text("id".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
        (Value::Text("s".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
        (Value::Text("t".into()), Value::Integer(42)),
        (Value::Text("e".into()), Value::Text(event.into())),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    if let Some(body) = body {
        map.insert(Value::Text("b".into()), body);
    }
    serde_cbor::to_vec(&Value::Map(map)).unwrap()
}

#[test]
fn decodes_notifications_without_payload() {
    for event in ["cd", "cl"] {
        for body in [None, Some(Value::Null)] {
            let bytes = wire(event, body);
            let decoded: Notification = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded.timestamp(), 42);
        }
    }
}

#[test]
fn decodes_session_key_rotation() {
    let bytes = wire("csr", Some(Value::Bytes(vec![7; 64])));
    let decoded: Notification = serde_cbor::from_slice(&bytes).unwrap();
    assert_eq!(decoded.timestamp(), 42);
}
