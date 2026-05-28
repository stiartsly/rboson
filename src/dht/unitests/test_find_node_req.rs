use crate::{
    Id,
    dht::msg::{
        lookup_req::LookupRequest,
        find_node_req::FindNodeRequest,
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let nodeid = Id::random();
        let req = FindNodeRequest::new(
            nodeid.clone(),
            true,
            false,
            true,
        );

        let encoded = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded = serde_cbor::from_slice::<FindNodeRequest>(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &nodeid);
        assert_eq!(decoded.want(), 0x05);

        assert!(decoded.want4());
        assert!(!decoded.want6());
        assert!(decoded.want_token());
    }

    #[test]
    fn test_serde_no_token() {
        let nodeid = Id::random();
        let req = FindNodeRequest::new(
            nodeid.clone(),
            true,
            true,
            false,
        );

        let encoded = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded = serde_cbor::from_slice::<FindNodeRequest>(&encoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &nodeid);
        assert_eq!(decoded.want(), 0x03);

        assert!(decoded.want4());
        assert!(decoded.want6());
        assert!(!decoded.want_token());

    }
}
