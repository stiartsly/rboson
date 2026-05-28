use std::net::SocketAddr;
use crate::{
    Id,
    Network,
    NodeInfo,
    dht::msg::{
        lookup_rsp::LookupResponse,
        find_node_rsp::FindNodeResponse,
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

        let rsp = FindNodeResponse::new(
            Some(vec![node4.clone()]),
            None,
            0,
        );

        let nodes4 = vec![node4.clone()];
        assert_eq!(rsp.nodes(Network::IPv4), Some(nodes4.as_slice()));
        assert_eq!(rsp.nodes4(), Some(nodes4.as_slice()));
        assert_eq!(rsp.nodes6(), None);
        assert_eq!(rsp.token(), 0);

        let nodeid = Id::random();
        let addr6 = "[::1]:29001".parse::<SocketAddr>().unwrap();
        let node6 = NodeInfo::new(nodeid, addr6);

        let rsp = FindNodeResponse::new(
            None,
            Some(vec![node6.clone()]),
            0,
        );

        let nodes6 = vec![node6.clone()];
        assert_eq!(rsp.nodes6(), Some(nodes6.as_slice()));
        assert_eq!(rsp.nodes4(), None);
        assert_eq!(rsp.token(), 0);

        let rsp = FindNodeResponse::new(
            Some(vec![node4.clone()]),
            Some(vec![node6.clone()]),
            1,
        );

        let nodes4 = [node4.clone()];
        let nodes6 = [node6.clone()];
        assert_eq!(rsp.nodes4(), Some(nodes4.as_slice()));
        assert_eq!(rsp.nodes6(), Some(nodes6.as_slice()));
        assert_eq!(rsp.token(), 1);
    }

    #[test]
    fn test_serde() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let node4 = NodeInfo::new(nodeid.clone(), addr);

        let nodeid = Id::random();
        let addr = "[::1]:29001".parse::<SocketAddr>().unwrap();
        let node6 = NodeInfo::new(nodeid.clone(), addr);
        let token = 12345;

        let rsp = FindNodeResponse::new(
            Some(vec![node4.clone()]),
            Some(vec![node6.clone()]),
            token
        );

        let necoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded = serde_cbor::from_slice::<FindNodeResponse>(&necoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), token);
        assert_eq!(decoded.nodes4().is_some(), true);
        assert_eq!(decoded.nodes6().is_some(), true);

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 1);
        assert_eq!(nodes4[0], node4);

        let nodes6 = decoded.nodes6().unwrap();
        assert_eq!(nodes6.len(), 1);
        assert_eq!(nodes6[0], node6);
    }

    #[test]
    fn test_serde_nodes4() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let node1 = NodeInfo::new(nodeid.clone(), addr);

        let nodeid = Id::random();
        let addr = "127.0.0.1:29002".parse::<SocketAddr>().unwrap();
        let node2 = NodeInfo::new(nodeid.clone(), addr);
        let token = 12345;

        let rsp = FindNodeResponse::new(
            Some(vec![node1.clone(), node2.clone()]),
            None,
            token
        );

        let necoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded = serde_cbor::from_slice::<FindNodeResponse>(&necoded)
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), token);
        assert_eq!(decoded.nodes4().is_some(), true);
        assert_eq!(decoded.nodes6().is_some(), false);

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 2);
        assert_eq!(nodes4[0], node1);
        assert_eq!(nodes4[1], node2);
    }
}
