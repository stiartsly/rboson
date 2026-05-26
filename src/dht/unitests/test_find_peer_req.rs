use crate::{
    Id,
    dht::msg::{
        lookup_req::LookupRequest,
        find_peer_req::FindPeerRequest,
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_cbor() {
        let peerid = Id::random();
        let expected_seq = 5;
        let expected_count = 10;
        let req = FindPeerRequest::new(
            peerid,
            true,
            false,
            expected_seq,
            expected_count
        );

        let cbor = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded: FindPeerRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &peerid);
        assert_eq!(decoded.want4(), true);
        assert_eq!(decoded.want6(), false);
        assert_eq!(decoded.expected_seq(), expected_seq);
        assert_eq!(decoded.expected_count(), expected_count);
    }

    #[test]
    fn test_serde_without_cas() {
        let peerid = Id::random();
        let req = FindPeerRequest::new(
            peerid,
            false,
            true,
            -1,
            3,
        );

        let cbor = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded: FindPeerRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.target(), &peerid);
        assert_eq!(decoded.want4(), false);
        assert_eq!(decoded.want6(), true);
        assert_eq!(decoded.want_token(), false);
        assert_eq!(decoded.expected_seq(), -1);
        assert_eq!(decoded.expected_count(), 3);
    }
}
