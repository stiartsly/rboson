use crate::{
    Id,
    Value,
    cryptobox::Nonce,
    dht::msg::store_value_req::StoreValueRequest,
};

fn make_value() -> Value {
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
    fn test_serde() {
        let value = make_value();
        let req = StoreValueRequest::new(value.clone(), 42, 11);
        assert_eq!(req.token(), 42);
        assert_eq!(req.expected_seq(), 11);
        assert_eq!(req.value(), &value);

        let encoded = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        println!("encoded: {}", hex::encode(&encoded));
        let decoded: StoreValueRequest = serde_cbor::from_slice(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), 11);
        assert_eq!(decoded.value(), &value);
    }

    #[test]
    fn test_serde_no_expected_seq() {
        let value = make_value();
        let req = StoreValueRequest::new(value.clone(), 42, -1);

        assert_eq!(req.token(), 42);
        assert_eq!(req.expected_seq(), -1);
        assert_eq!(req.value(), &value);

        let encoded = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        println!("encoded: {}", hex::encode(&encoded));
        let decoded: StoreValueRequest = serde_cbor::from_slice(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), -1);
        assert_eq!(decoded.value(), &value);
    }
}
