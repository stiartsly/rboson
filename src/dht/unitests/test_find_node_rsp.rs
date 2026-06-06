use std::net::SocketAddr;
use crate::{
    Id,
    Network,
    NodeInfo,
    dht::msg::{LookupResponse, FindNodeResponse}
};

fn make_node_info4_with_port(port: u16) -> NodeInfo {
    let addr = format!("127.0.0.1:{}", port).parse::<SocketAddr>().unwrap();
    NodeInfo::new(Id::random(), addr)
}

fn make_node_info4() -> NodeInfo {
    make_node_info4_with_port(39001)
}

fn make_node_info6() -> NodeInfo {
    let addr = format!("[::1]:{}", 39001).parse::<SocketAddr>().unwrap();
    NodeInfo::new(Id::random(), addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_nodes() {
        let node4 = make_node_info4();
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

        let node6 = make_node_info6();
        let rsp = FindNodeResponse::new(
            None,
            Some(vec![node6.clone()]),
            0,
        );

        let nodes6 = vec![node6.clone()];
        assert_eq!(rsp.nodes6(), Some(nodes6.as_slice()));
        assert_eq!(rsp.nodes4(), None);
        assert_eq!(rsp.token(), 0);


        let node4 = make_node_info4();
        let node6 = make_node_info6();
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
        let node4 = make_node_info4();
        let node6 = make_node_info6();
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
    fn test_serde_with_nodes() {
        let node1 = make_node_info4_with_port(29001);
        let node2 = make_node_info4_with_port(29002);
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
