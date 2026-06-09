use crate::dht::msg::error::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let err = Error::new(500, "example error");
        assert_eq!(err.code(), 500);
        assert_eq!(err.description(), "example error");

        let encoded = serde_cbor::to_vec(&err)
            .expect("Serialization failed");
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded = serde_cbor::from_slice::<Error>(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.code(), 500);
        assert_eq!(decoded.description(), "example error");
    }

    #[test]
    fn test_serde_with_empty_description() {
        let err = Error::new(404, "");
        assert_eq!(err.code(), 404);
        assert_eq!(err.description(), "");

        let encoded = serde_cbor::to_vec(&err)
            .expect("Serialization failed");
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded = serde_cbor::from_slice::<Error>(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.code(), 404);
        assert_eq!(decoded.description(), "");
    }
}
