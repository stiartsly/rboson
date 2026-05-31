use crate::dht::msg::error::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let err = Error::new(500, "boom".to_string());
        assert_eq!(err.code(), 500);
        assert_eq!(err.description(), "boom");

        let encoded = serde_cbor::to_vec(&err)
            .expect("Serialization failed");
        let decoded: Error = serde_cbor::from_slice(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.code(), 500);
        assert_eq!(decoded.description(), "boom");
    }

    #[test]
    fn test_serde_with_empty_msg() {
        let err = Error::new(404, String::new());
        assert_eq!(err.code(), 404);
        assert_eq!(err.description(), "");

        let encoded = serde_cbor::to_vec(&err)
            .expect("Serialization failed");
        let decoded: Error = serde_cbor::from_slice(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.code(), 404);
        assert_eq!(decoded.description(), "");
    }
}
