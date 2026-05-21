use crate::{
    Id,
    Value,
    cryptobox::Nonce,
    dht::msg::store_value_req::StoreValueRequest,
};

fn sample_value() -> Value {
    Value::packed(
        Some(Id::random()),
        None,
        Some(Nonce::random()),
        Some(vec![9; 64]),
        vec![1, 2, 3, 4],
        7,
    )
}

#[cfg(test)]
mod tests {
        use super::*;
    #[test]
    fn test_accessors() {
        let value = sample_value();
        let req = StoreValueRequest::new(value.clone(), 42, 11);

        assert_eq!(req.token(), 42);
        assert_eq!(req.expected_seq(), 11);
        assert_eq!(req.value(), &value);
    }

    #[test]
    fn test_serde() {
        let value = sample_value();
        let req = StoreValueRequest::new(value.clone(), 42, 11);

        let cbor = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded: StoreValueRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), 11);
        assert_eq!(decoded.value(), &value);
    }

    #[test]
    fn test_serde_without_cas() {
        let value = sample_value();
        let req = StoreValueRequest::new(value.clone(), 42, -1);

        let cbor = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded: StoreValueRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), -1);
        assert_eq!(decoded.value(), &value);
    }

    #[test]
    fn test_display() {
        let value = sample_value();
        let req = StoreValueRequest::new(value.clone(), 42, 11);

        assert_eq!(format!("{}", req), format!("tok:42,v:[{}],cas:11", value));
    }

    #[test]
    fn test_display_without_cas() {
        let value = sample_value();
        let req = StoreValueRequest::new(value.clone(), 42, -1);

        assert_eq!(format!("{}", req), format!("tok:42,v:[{}]", value));
    }
}
