use serde_cbor::Value as CborValue;
use crate::{
    Id,
    dht::msg::msg
};

#[cfg(test)]
mod tests {
    use super::*;

    fn check_key<'a>(value: &'a CborValue, key: &str) -> Option<&'a CborValue> {
        let CborValue::Map(entries) = value else {
            return None;
        };

        entries.iter().find_map(|(entry_key, entry_value)| {
            match entry_key {
                CborValue::Text(text) if text == key => Some(entry_value),
                _ => None,
            }
        })
    }

    #[test]
    fn test_serde_find_value_request() {
        let target = Id::random();
        let msg = msg::find_value_request(target.clone(), true, false, 7);

        let encoded = serde_cbor::to_vec(&msg)
            .expect("message serialization failed");
        let decoded: CborValue = serde_cbor::from_slice(&encoded)
            .expect("message cbor decoding failed");

        let body = check_key(&decoded, "q").expect("missing request body");
        let target_value = check_key(body, "t").expect("missing target field");
        let want_value = check_key(body, "w").expect("missing want field");

        assert!(matches!(body, CborValue::Map(_)));
        assert_eq!(serde_cbor::value::from_value::<Id>(target_value.clone()).unwrap(), target);
        assert_eq!(serde_cbor::value::from_value::<i32>(want_value.clone()).unwrap(), 1);
        assert!(check_key(body, "FindValueRequest").is_none());
    }
}
