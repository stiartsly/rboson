use std::net::SocketAddr;
use crate::{
    Id,
    NodeInfo,
    PeerBuilder,
    signature,
    dht::msg::{
        find_peer_rsp::FindPeerResponse,
        lookup_rsp::LookupResponse
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsp_with_nodes() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let node = NodeInfo::new(nodeid, addr);

        let rsp = FindPeerResponse::with_nodes(
            Some(vec![node.clone()]),
            None,
        );

        assert_eq!(rsp.nodes4(), Some([node.clone()].as_slice()));
        assert_eq!(rsp.nodes6(), None);
        assert_eq!(rsp.peers(), None);
    }

    #[test]
    fn test_rsp_with_peers() {
        let keypair = signature::KeyPair::random();
        let peer = PeerBuilder::new("tcp://192.168.1.1:8080")
            .with_key(keypair)
            .build()
            .expect("Failed to build peer");

        let rsp = FindPeerResponse::with_peers(vec![peer.clone()]);

        assert_eq!(rsp.nodes4(), None);
        assert_eq!(rsp.nodes6(), None);
        assert_eq!(rsp.peers(), Some([peer].as_slice()));
    }

    #[test]
    fn test_serde_with_nodes() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let ni4 = NodeInfo::new(nodeid.clone(), addr);

        let nodeid = Id::random();
        let addr = "[::1]:29001".parse::<SocketAddr>().unwrap();
        let ni6 = NodeInfo::new(nodeid.clone(), addr);

        let rsp = FindPeerResponse::with_nodes(
            Some(vec![ni4.clone()]),
            Some(vec![ni6.clone()])
        );

        assert_eq!(rsp.nodes4().is_some(), true);
        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().is_some(), true);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindPeerResponse = serde_cbor::from_slice(cbor.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 0);
        assert_eq!(decoded.nodes4().is_some(), true);
        assert_eq!(decoded.nodes6().is_some(), true);

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 1);
        assert_eq!(nodes4[0], ni4);

        let nodes6 = decoded.nodes6().unwrap();
        assert_eq!(nodes6.len(), 1);
        assert_eq!(nodes6[0], ni6);
    }

    #[test]
    fn test_serde_with_peer() {
        let keypair = signature::KeyPair::random();
        let endpoint = "tcp://192.168.1.1:8080";
        let peer = PeerBuilder::new(endpoint)
            .with_key(keypair.clone())
            .build()
            .expect("Failed to build peer");

        let rsp = FindPeerResponse::with_peers(vec![peer.clone()]);

        assert_eq!(rsp.nodes4().is_none(), true);
        assert_eq!(rsp.nodes6().is_none(), true);
        assert_eq!(rsp.token(), 0);
        assert_eq!(rsp.peers().is_some(), true);
        assert_eq!(rsp.peers().unwrap().len(), 1);

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindPeerResponse = serde_cbor::from_slice(cbor.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.nodes4().is_none(), true);
        assert_eq!(decoded.nodes6().is_none(), true);
        assert_eq!(decoded.token(), 0);

        assert_eq!(decoded.peers().is_some(), true);
        assert_eq!(decoded.peers().unwrap().len(), 1);

        let decoded_peer = &decoded.peers().unwrap()[0];
        assert_eq!(decoded_peer.id(), peer.id());
        assert_eq!(decoded_peer.endpoint(), peer.endpoint());
        assert_eq!(decoded_peer.signature(), peer.signature());
        assert_eq!(decoded_peer.nonce(), peer.nonce());
    }

    #[test]
    fn test_serde_with_multiple_peers() {
        let keypair = signature::KeyPair::random();
        let peer1 = PeerBuilder::new("tcp://192.168.1.1:8080")
            .with_key(keypair.clone())
            .build()
            .expect("Failed to build peer1");

        let peer2 = PeerBuilder::new("tcp://192.168.1.2:9090")
            .with_key(keypair.clone())
            .with_extra(&[5, 6, 7])
            .build()
            .expect("Failed to build peer2");

        let rsp = FindPeerResponse::with_peers(
            vec![peer1.clone(), peer2.clone()]
        );

        assert_eq!(rsp.nodes4().is_none(), true);
        assert_eq!(rsp.nodes6().is_none(), true);
        assert_eq!(rsp.token(), 0);

        assert_eq!(rsp.peers().is_some(), true);
        assert_eq!(rsp.peers().unwrap().len(), 2);

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindPeerResponse = serde_cbor::from_slice(cbor.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.peers().unwrap().len(), 2);

        let decoded_peer1 = &decoded.peers().unwrap()[0];
        assert_eq!(decoded_peer1.id(), peer1.id());
        assert_eq!(decoded_peer1.endpoint(), peer1.endpoint());
        assert_eq!(decoded_peer1.signature(), peer1.signature());
        assert_eq!(decoded_peer1.nonce(), peer1.nonce());

        let decoded_peer2 = &decoded.peers().unwrap()[1];
        assert_eq!(decoded_peer2.id(), peer2.id());
        assert_eq!(decoded_peer2.endpoint(), peer2.endpoint());
        assert_eq!(decoded_peer2.signature(), peer2.signature());
        assert_eq!(decoded_peer2.nonce(), peer2.nonce());
    }
}
