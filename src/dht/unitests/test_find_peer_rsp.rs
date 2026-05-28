use std::net::SocketAddr;
use crate::{
    Id,
    Network,
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
    fn test_with_nodes() {
        let nodeid = Id::random();
        let addr4 = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let node4 = NodeInfo::new(nodeid, addr4);

        let nodeid = Id::random();
        let addr6 = "[::1]:29001".parse::<SocketAddr>().unwrap();
        let node6 = NodeInfo::new(nodeid, addr6);

        let rsp = FindPeerResponse::with_nodes(
            Some(vec![node4.clone()]),
            Some(vec![node6.clone()])
        );

        assert!(rsp.nodes4().is_some());
        assert!(rsp.nodes6().is_some());
        assert!(rsp.peers().is_none());

        assert_eq!(rsp.peers(), None);
        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        assert_eq!(rsp.nodes4(), Some([node4.clone()].as_slice()));
        assert_eq!(rsp.nodes6(), Some([node6.clone()].as_slice()));
        assert_eq!(rsp.nodes4(), rsp.nodes(Network::IPv4));
        assert_eq!(rsp.nodes6(), rsp.nodes(Network::IPv6));
    }

    #[test]
    fn test_with_peers() {
        let keypair = signature::KeyPair::random();
        let peer1 = PeerBuilder::new("tcp://192.168.1.1:8080")
            .with_key(keypair.clone())
            .with_fingerprint(1)
            .build()
            .expect("Failed to build peer");

        let peer2 = PeerBuilder::new("tcp://192.168.1.1:8081")
            .with_key(keypair.clone())
            .with_fingerprint(2)
            .build()
            .expect("Failed to build peer");

        let peers = vec![peer1.clone(), peer2.clone()];
        let rsp = FindPeerResponse::with_peers(peers.clone());

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.peers().is_some());

        assert_eq!(rsp.peers().unwrap().len(), peers.len());
        assert_eq!(rsp.peers(), Some(peers.as_slice()));
        assert_eq!(rsp.token(), 0);

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

        assert!(rsp.nodes4().is_some());
        assert!(rsp.nodes6().is_some());
        assert!(rsp.peers().is_none());

        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        let encoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindPeerResponse = serde_cbor::from_slice(encoded.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 0);
        assert!(decoded.nodes4().is_some());
        assert!(decoded.nodes6().is_some());
        assert!(decoded.peers().is_none());

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 1);
        assert_eq!(nodes4[0], ni4);

        let nodes6 = decoded.nodes6().unwrap();
        assert_eq!(nodes6.len(), 1);
        assert_eq!(nodes6[0], ni6);
    }

    #[test]
    fn test_serde_with_apeer() {
        let keypair = signature::KeyPair::random();
        let endpoint = "tcp://192.168.1.1:8080";
        let peer = PeerBuilder::new(endpoint)
            .with_key(keypair.clone())
            .build()
            .expect("Failed to build peer");

        let rsp = FindPeerResponse::with_peers(vec![peer.clone()]);

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.peers().is_some());

        assert_eq!(rsp.token(), 0);
        assert_eq!(rsp.peers().unwrap().len(), 1);
        assert_eq!(rsp.peers().unwrap()[0], peer.clone());

        let encoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindPeerResponse = serde_cbor::from_slice(encoded.as_slice())
            .expect("Deserialization failed");

        assert!(decoded.nodes4().is_none());
        assert!(decoded.nodes6().is_none());
        assert!(decoded.peers().is_some());

        assert_eq!(decoded.token(), 0);
        assert_eq!(decoded.peers().unwrap().len(), 1);

        let decoded_peer = &decoded.peers().unwrap()[0];
        assert_eq!(decoded_peer.id(), peer.id());
        assert_eq!(decoded_peer.endpoint(), peer.endpoint());
        assert_eq!(decoded_peer.signature(), peer.signature());
        assert_eq!(decoded_peer.nonce(), peer.nonce());
    }

    #[test]
    fn test_serde_with_peers() {
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

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.peers().is_some());

        assert_eq!(rsp.token(), 0);
        assert_eq!(rsp.peers().unwrap().len(), 2);

        let encoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindPeerResponse = serde_cbor::from_slice(encoded.as_slice())
            .expect("Deserialization failed");

        assert!(decoded.nodes4().is_none());
        assert!(decoded.nodes6().is_none());
        assert!(decoded.peers().is_some());

        assert_eq!(decoded.token(), 0);
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
