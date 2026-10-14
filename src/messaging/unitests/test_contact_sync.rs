use super::*;
use crate::Id;
use serde_cbor::Value;

#[test]
fn contact_sync_roundtrips_with_java_wire_format() {
    let id = Id::random();
    let syncs = [
        ContactSync::up_to_date(7).unwrap(),
        ContactSync::delta(7, vec![ContactMutation::clear(6)]).unwrap(),
        ContactSync::snapshot(7, vec![OpaqueContact::new(id, 7, Value::Bytes(vec![1, 2]))]).unwrap(),
        ContactSync::snapshot(7, Vec::new()).unwrap(),
    ];
    for (sync, kind) in syncs.iter().zip([0, 1, 2, 2]) {
        let bytes = serde_cbor::to_vec(sync).unwrap();
        let Value::Map(wire) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
            panic!("Contact sync must be a map");
        };
        assert_eq!(wire.get(&Value::Text("v".into())), Some(&Value::Integer(7)));
        assert_eq!(wire.get(&Value::Text("t".into())), Some(&Value::Integer(kind)));
        assert_eq!(wire.contains_key(&Value::Text("d".into())), !sync.mutations().is_empty());
        assert_eq!(wire.contains_key(&Value::Text("s".into())), !sync.contacts().is_empty());
        let parsed: ContactSync = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(parsed.kind(), sync.kind());
        assert_eq!(parsed.revision(), sync.revision());
        assert_eq!(serde_cbor::value::to_value(&parsed).unwrap(), Value::Map(wire));
    }
}

#[test]
fn contact_sync_normalizes_absent_null_and_empty_lists() {
    for kind in [0, 2] {
        for lists in ["", r#","d":null,"s":null"#, r#","d":[],"s":[]"#] {
            let sync: ContactSync = serde_json::from_str(&format!(r#"{{"v":0,"t":{kind}{lists}}}"#)).unwrap();
            assert!(sync.mutations().is_empty());
            assert!(sync.contacts().is_empty());
            assert_eq!(serde_json::to_value(sync).unwrap(), serde_json::json!({"v": 0, "t": kind}));
        }
    }
}

#[test]
fn contact_sync_rejects_invalid_payloads() {
    for invalid in [
        r#"{"t":0}"#,
        r#"{"v":0}"#,
        r#"{"v":-1,"t":0}"#,
        r#"{"v":0,"t":3}"#,
        r#"{"v":0,"t":-1}"#,
        r#"{"v":0,"t":"0"}"#,
        r#"{"v":0,"t":1}"#,
        r#"{"v":0,"t":1,"d":null}"#,
        r#"{"v":0,"t":1,"d":[]}"#,
        r#"{"v":1,"t":0,"d":[{"v":0,"op":"c"}]}"#,
        r#"{"v":1,"t":2,"d":[{"v":0,"op":"c"}]}"#,
        r#"{"v":1,"t":1,"d":[{"v":0,"op":"unknown"}]}"#,
        r#"{"v":1,"t":1,"d":[{"v":0,"op":"a"}]}"#,
    ] {
        assert!(serde_json::from_str::<ContactSync>(invalid).is_err(), "{invalid}");
    }
    assert!(ContactSync::up_to_date(-1).is_err());
    assert!(ContactSync::delta(1, Vec::new()).is_err());
    assert!(ContactSync::snapshot(-1, Vec::new()).is_err());
    let contact = || OpaqueContact::new(Id::random(), 1, Value::Bytes(vec![1]));
    assert!(ContactSync::new(1, SyncType::UpToDate, vec![], vec![contact()]).is_err());
    assert!(ContactSync::new(1, SyncType::Delta, vec![], vec![contact()]).is_err());
    assert!(ContactSync::new(1, SyncType::Delta, vec![ContactMutation::clear(0)], vec![contact()]).is_err());
}

#[test]
fn contact_sync_type_values_match_java() {
    for (value, kind) in [(0, SyncType::UpToDate), (1, SyncType::Delta), (2, SyncType::Snapshot)] {
        assert_eq!(SyncType::try_from(value).unwrap(), kind);
        assert_eq!(kind.value(), value);
    }
    assert!(SyncType::try_from(-1).is_err());
    assert!(SyncType::try_from(3).is_err());
}
