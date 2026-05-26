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
    fn test_new1() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let node = NodeInfo::new(nodeid, addr);

        let rsp = FindNodeResponse::new(
            Some(vec![node.clone()]),
            None,
            29001,
        );

        assert_eq!(rsp.nodes4(), Some([node].as_slice()));
        assert_eq!(rsp.nodes6(), None);
        assert_eq!(rsp.token(), 29001);
    }

    #[test]
    fn test_new2() {
        let nodeid = Id::random();
        let addr4 = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let node4 = NodeInfo::new(nodeid, addr4);

        let rsp = FindNodeResponse::new(
            Some(vec![node4.clone()]),
            None,
            0,
        );

        assert_eq!(rsp.nodes(Network::IPv4), Some([node4.clone()].as_slice()));
        assert_eq!(rsp.nodes4(), Some([node4.clone()].as_slice()));
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

        assert_eq!(rsp.nodes6(), Some([node6.clone()].as_slice()));
        assert_eq!(rsp.nodes4(), None);
        assert_eq!(rsp.token(), 0);

        let rsp = FindNodeResponse::new(
            Some(vec![node4.clone()]),
            Some(vec![node6.clone()]),
            1,
        );

        assert_eq!(rsp.nodes4(), Some([node4.clone()].as_slice()));
        assert_eq!(rsp.nodes6(), Some([node6.clone()].as_slice()));
        assert_eq!(rsp.token(), 1);
    }

    #[test]
    fn test_serde_default() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let ni = NodeInfo::new(nodeid.clone(), addr);
        let token = 29001;

        let rsp = FindNodeResponse::new(
            Some(vec![ni.clone()]),
            None,
            token
        );

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindNodeResponse = serde_cbor::from_slice(cbor.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), token);
        assert_eq!(decoded.nodes4().is_some(), true);
        assert_eq!(decoded.nodes6().is_some(), false);

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 1);
        assert_eq!(nodes4[0], ni);
    }

    #[test]
    fn test_serde_with_ipv6() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let ni4 = NodeInfo::new(nodeid.clone(), addr);

        let nodeid = Id::random();
        let addr = "[::1]:29001".parse::<SocketAddr>().unwrap();
        let ni6 = NodeInfo::new(nodeid.clone(), addr);
        let token = 29001;

        let rsp = FindNodeResponse::new(
            Some(vec![ni4.clone()]),
            Some(vec![ni6.clone()]),
            token
        );

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindNodeResponse = serde_cbor::from_slice(cbor.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), token);
        assert_eq!(decoded.nodes4().is_some(), true);
        assert_eq!(decoded.nodes6().is_some(), true);

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 1);
        assert_eq!(nodes4[0], ni4);

        let nodes6 = decoded.nodes6().unwrap();
        assert_eq!(nodes6.len(), 1);
        assert_eq!(nodes6[0], ni6);
    }
}
