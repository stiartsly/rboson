use crate::{
    Id,
    PeerInfo,
    dht::msg::announce_peer_req::AnnouncePeerRequest,
};

fn sample_peer() -> PeerInfo {
    PeerInfo::packed(
        Id::random(),
        vec![7; PeerInfo::NONCE_BYTES],
        5,
        None,
        None,
        vec![9; 64],
        123456,
        "127.0.0.1:39001".to_string(),
        Some(vec![1, 2, 3]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessors() {
        let peer = sample_peer();
        let req = AnnouncePeerRequest::new(peer.clone(), 42, 8);

        assert_eq!(req.token(), 42);
        assert_eq!(req.expected_seq(), 8);
        assert_eq!(req.peer(), &peer);
    }

    #[test]
    fn test_serde() {
        let peer = sample_peer();
        let req = AnnouncePeerRequest::new(peer.clone(), 42, 8);

        let cbor = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded: AnnouncePeerRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), 8);
        assert_eq!(decoded.peer(), &peer);
    }

    #[test]
    fn test_serde_without_cas() {
        let peer = sample_peer();
        let req = AnnouncePeerRequest::new(peer.clone(), 42, -1);

        let cbor = serde_cbor::to_vec(&req)
            .expect("Serialization failed");
        let decoded: AnnouncePeerRequest = serde_cbor::from_slice(&cbor)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), -1);
        assert_eq!(decoded.peer(), &peer);
    }

    #[test]
    fn test_display() {
        let peer = sample_peer();
        let req = AnnouncePeerRequest::new(peer.clone(), 42, 8);

        let str = format!("{}", req);
        assert_eq!(str, format!("tok:42,peer:[{}],cas:8", peer));
    }

    #[test]
    fn test_display_without_cas() {
        let peer = sample_peer();
        let req = AnnouncePeerRequest::new(peer.clone(), 42, -1);

        let str = format!("{}", req);
        assert_eq!(str, format!("tok:42,peer:[{}]", peer));
    }
}
