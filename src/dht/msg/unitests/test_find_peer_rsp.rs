use crate::{
    dht::msg::{find_peer_rsp::FindPeerResponse, lookup_rsp::LookupResponse},
    signature, Id, Network, NodeInfo, PeerBuilder, PeerInfo,
};
use std::net::SocketAddr;

fn make_node_info4() -> NodeInfo {
    let addr = format!("127.0.0.1:{}", 39011)
        .parse::<SocketAddr>()
        .unwrap();
    NodeInfo::new(Id::random(), addr)
}

fn make_node_info6() -> NodeInfo {
    let addr = format!("[::1]:{}", 39011).parse::<SocketAddr>().unwrap();
    NodeInfo::new(Id::random(), addr)
}

fn make_peer(port: u16) -> PeerInfo {
    let keypair = signature::KeyPair::random();
    let endpoint = format!("tcp://192.168.1.1:{}", port);
    PeerBuilder::new(&endpoint)
        .with_key(keypair.clone())
        .with_fingerprint(port as u64)
        .build()
        .expect("Failed to build peer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_nodes() {
        let node4 = make_node_info4();
        let node6 = make_node_info6();

        let rsp =
            FindPeerResponse::with_nodes(Some(vec![node4.clone()]), Some(vec![node6.clone()]));

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
        let peer1 = make_peer(8080);
        let peer2 = make_peer(8081);

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
        let ni4 = make_node_info4();
        let ni6 = make_node_info6();

        let rsp = FindPeerResponse::with_nodes(Some(vec![ni4.clone()]), Some(vec![ni6.clone()]));

        assert!(rsp.nodes4().is_some());
        assert!(rsp.nodes6().is_some());
        assert!(rsp.peers().is_none());

        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        let encoded = serde_cbor::to_vec(&rsp).expect("Serialization failed");
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded: FindPeerResponse =
            serde_cbor::from_slice(encoded.as_slice()).expect("Deserialization failed");

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
    fn test_serde_with_only_ipv4_nodes() {
        let node4 = make_node_info4();
        let response = FindPeerResponse::with_nodes(Some(vec![node4.clone()]), None);

        let encoded = serde_cbor::to_vec(&response).expect("Serialization failed");
        let decoded: FindPeerResponse =
            serde_cbor::from_slice(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.token(), 0);
        assert_eq!(decoded.nodes4(), Some([node4].as_slice()));
        assert!(decoded.nodes6().is_none());
        assert!(decoded.peers().is_none());
    }

    #[test]
    fn test_serde_with_only_ipv6_nodes() {
        let node6 = make_node_info6();
        let response = FindPeerResponse::with_nodes(None, Some(vec![node6.clone()]));

        let encoded = serde_cbor::to_vec(&response).expect("Serialization failed");
        let decoded: FindPeerResponse =
            serde_cbor::from_slice(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.token(), 0);
        assert!(decoded.nodes4().is_none());
        assert_eq!(decoded.nodes6(), Some([node6].as_slice()));
        assert!(decoded.peers().is_none());
    }

    #[test]
    fn test_serde_with_peer() {
        let peer = make_peer(8080);
        let rsp = FindPeerResponse::with_peers(vec![peer.clone()]);

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.peers().is_some());

        assert_eq!(rsp.token(), 0);
        assert_eq!(rsp.peers().unwrap().len(), 1);
        assert_eq!(rsp.peers().unwrap()[0], peer.clone());

        let encoded = serde_cbor::to_vec(&rsp).expect("Serialization failed");
        let encoded_value: serde_cbor::Value =
            serde_cbor::from_slice(&encoded).expect("Response should decode as CBOR");
        let fields = match encoded_value {
            serde_cbor::Value::Map(fields) => fields,
            _ => panic!("Find peer response should encode as a CBOR map"),
        };
        let peers = fields.iter().find_map(|(key, value)| match key {
            serde_cbor::Value::Text(name) if name == "p" => Some(value),
            _ => None,
        });
        assert!(
            matches!(peers, Some(serde_cbor::Value::Bytes(_))),
            "peers should be encoded as a packed CBOR byte string"
        );
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded: FindPeerResponse =
            serde_cbor::from_slice(encoded.as_slice()).expect("Deserialization failed");

        assert!(decoded.nodes4().is_none());
        assert!(decoded.nodes6().is_none());
        assert!(decoded.peers().is_some());

        assert_eq!(decoded.token(), 0);
        assert_eq!(decoded.peers().unwrap().len(), 1);

        let decoded_peer = &decoded.peers().unwrap()[0];
        assert_eq!(decoded_peer.id(), peer.id());
        assert_eq!(decoded_peer.endpoint(), peer.endpoint());
        assert_eq!(decoded_peer.signature(), peer.signature());
    }

    #[test]
    fn test_deserialize_inline_peers() {
        #[derive(serde::Serialize)]
        struct InlinePeerResponse {
            #[serde(rename = "tok")]
            token: i32,
            #[serde(rename = "p")]
            peers: Vec<PeerInfo>,
        }

        let peer = make_peer(8080);
        let encoded = serde_cbor::to_vec(&InlinePeerResponse {
            token: 0,
            peers: vec![peer.clone()],
        })
        .expect("Inline peer response serialization failed");

        let decoded: FindPeerResponse =
            serde_cbor::from_slice(&encoded).expect("Inline peer response deserialization failed");

        assert_eq!(decoded.token(), 0);
        let expected_peer = peer.without_private_key();
        assert_eq!(decoded.peers(), Some([expected_peer].as_slice()));
    }

    #[test]
    fn test_serde_with_peers() {
        let peer1 = make_peer(8080);
        let peer2 = make_peer(8081);
        let rsp = FindPeerResponse::with_peers(vec![peer1.clone(), peer2.clone()]);

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.peers().is_some());

        assert_eq!(rsp.token(), 0);
        assert_eq!(rsp.peers().unwrap().len(), 2);

        let encoded = serde_cbor::to_vec(&rsp).expect("Serialization failed");
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded: FindPeerResponse =
            serde_cbor::from_slice(encoded.as_slice()).expect("Deserialization failed");

        assert!(decoded.nodes4().is_none());
        assert!(decoded.nodes6().is_none());
        assert!(decoded.peers().is_some());

        assert_eq!(decoded.token(), 0);
        assert_eq!(decoded.peers().unwrap().len(), 2);

        let decoded_peer1 = &decoded.peers().unwrap()[0];
        assert_eq!(decoded_peer1.id(), peer1.id());
        assert_eq!(decoded_peer1.endpoint(), peer1.endpoint());
        assert_eq!(decoded_peer1.signature(), peer1.signature());

        let decoded_peer2 = &decoded.peers().unwrap()[1];
        assert_eq!(decoded_peer2.id(), peer2.id());
        assert_eq!(decoded_peer2.endpoint(), peer2.endpoint());
        assert_eq!(decoded_peer2.signature(), peer2.signature());
    }
}
