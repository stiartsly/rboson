use std::collections::HashMap;
use serde_cbor::Value;

use super::{ContentFormat, PhotonContent};

fn headers() -> HashMap<String, Value> {
    HashMap::from([("X-Test".to_string(), Value::Text("value".to_string()))])
}

#[test]
fn constructors_match_their_formats() {
    let text = PhotonContent::text_with_headers(headers(), "hello");
    assert_eq!(text.format(), ContentFormat::Text);
    assert_eq!(text.as_text().unwrap(), "hello");
    assert_eq!(text.as_binary().unwrap(), b"hello");

    assert_eq!(PhotonContent::empty().as_text().unwrap(), "");

    let bare_binary = PhotonContent::binary(vec![9]);
    assert!(bare_binary.headers().is_empty());
    assert_eq!(bare_binary.as_binary().unwrap(), vec![9]);

    let binary = PhotonContent::binary_with_headers(headers(), vec![0, 1, 2]);
    assert_eq!(binary.format(), ContentFormat::Binary);
    assert_eq!(binary.as_binary().unwrap(), vec![0, 1, 2]);
    assert_eq!(binary.as_text().unwrap(), "\0\u{1}\u{2}");

    let object = PhotonContent::object(&vec![1, 2, 3]).unwrap();
    assert_eq!(object.format(), ContentFormat::Object);
    assert_eq!(object.as_list::<i32>().unwrap(), vec![1, 2, 3]);

    assert!(PhotonContent::new(headers(), ContentFormat::Text, Value::Bytes(vec![1])).is_err());
    assert!(PhotonContent::new(headers(), ContentFormat::Binary, Value::Text("x".into())).is_err());
}

#[test]
fn serialization_round_trips_and_validates_the_body() {
    let content = PhotonContent::binary_with_headers(headers(), vec![7, 8]);
    let encoded = content.serialize().unwrap();
    let decoded = PhotonContent::parse(&encoded).unwrap();
    assert_eq!(decoded.format(), ContentFormat::Binary);
    assert_eq!(decoded.headers(), content.headers());
    assert_eq!(decoded.body(), content.body());

    let invalid = serde_cbor::to_vec(&Value::Map(
        [
            (Value::Text("f".into()), Value::Text("t".into())),
            (Value::Text("b".into()), Value::Bytes(vec![1])),
        ]
        .into_iter()
        .collect(),
    ))
    .unwrap();
    assert!(PhotonContent::parse(&invalid).is_err());
}

#[test]
fn object_accessors_require_object_content() {
    let object = PhotonContent::object(&HashMap::from([("key", 7)])).unwrap();
    assert_eq!(
        object.as_map::<String, i32>().unwrap(),
        HashMap::from([("key".to_string(), 7)])
    );

    let text = PhotonContent::text("value");
    assert!(text.as_object::<String>().is_err());
    assert!(text.as_list::<i32>().is_err());
    assert!(text.as_map::<String, i32>().is_err());
}
