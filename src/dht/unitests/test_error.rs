use crate::dht::msg::error::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessors() {
        let err = Error::new(500, "boom".to_string());

        assert_eq!(err.code(), 500);
        assert_eq!(err.msg(), "boom");
    }

    #[test]
    fn test_serde() {
        let err = Error::new(500, "boom".to_string());

        let cbor = serde_cbor::to_vec(&err)
            .expect("Serialization failed");
        let decoded: Error = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.code(), 500);
        assert_eq!(decoded.msg(), "boom");
    }

    #[test]
    fn test_serde_with_empty_message() {
        let err = Error::new(404, String::new());

        let cbor = serde_cbor::to_vec(&err)
            .expect("Serialization failed");
        let decoded: Error = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.code(), 404);
        assert_eq!(decoded.msg(), "");
    }

    #[test]
    fn test_display() {
        let err = Error::new(500, "boom".to_string());
        assert_eq!(format!("{}", err), "c:500,m:boom");
    }
}
