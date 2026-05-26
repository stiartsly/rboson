use crate::{
    Id,
    dht::msg::{
        find_value_req::FindValueRequest,
        lookup_req::LookupRequest,
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_cbor() {
        let value_id = Id::random();
        let expected_seq = 7;
        let msg = FindValueRequest::new(
            value_id.clone(),
            true,
            false,
            expected_seq
        );

        let cbor = serde_cbor::to_vec(&msg)
            .expect("Serialization failed");
        let decoded: FindValueRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &value_id);
        assert_eq!(decoded.want4(), true);
        assert_eq!(decoded.want6(), false);
        assert_eq!(decoded.want_token(), false);
        assert_eq!(decoded.expected_seq(), expected_seq);
    }

    #[test]
    fn test_serde_without_cas() {
        let value_id = Id::random();

        let msg = FindValueRequest::new(
            value_id.clone(),
            false,
            true,
            -1,
        );

        let cbor = serde_cbor::to_vec(&msg)
            .expect("Serialization failed");
        let decoded: FindValueRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &value_id);
        assert_eq!(decoded.want4(), false);
        assert_eq!(decoded.want6(), true);
        assert_eq!(decoded.want_token(), false);
        assert_eq!(decoded.expected_seq(), -1);
    }
}
