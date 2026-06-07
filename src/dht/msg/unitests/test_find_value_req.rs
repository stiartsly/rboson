use crate::Id;
use crate::dht::msg::{
    FindValueRequest,
    LookupRequest,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let value_id = Id::random();
        let expected_seq = 7;
        let msg = FindValueRequest::new(
            value_id.clone(),
            true,
            false,
            expected_seq
        );

        assert_eq!(msg.target(), &value_id);
        assert_eq!(msg.expected_seq(), expected_seq);

        assert!(msg.want4());
        assert!(!msg.want6());
        assert!(!msg.want_token());

        let encoded = serde_cbor::to_vec(&msg)
            .expect("Serialization failed");
        let decoded: FindValueRequest = serde_cbor::from_slice(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &value_id);
        assert_eq!(decoded.expected_seq(), expected_seq);

        assert!(decoded.want4());
        assert!(!decoded.want6());
        assert!(!decoded.want_token());

    }

    #[test]
    fn test_serde_without_expected_seq() {
        let value_id = Id::random();

        let msg = FindValueRequest::new(
            value_id.clone(),
            false,
            true,
            -1,
        );

        assert_eq!(msg.target(), &value_id);
        assert_eq!(msg.expected_seq(), -1);

        assert!(!msg.want4());
        assert!(msg.want6());
        assert!(!msg.want_token());

        let encoded = serde_cbor::to_vec(&msg)
            .expect("Serialization failed");
        let decoded: FindValueRequest = serde_cbor::from_slice(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &value_id);
        assert_eq!(decoded.expected_seq(), -1);

        assert!(!decoded.want4());
        assert!(decoded.want6());
        assert!(!decoded.want_token());
    }
}
