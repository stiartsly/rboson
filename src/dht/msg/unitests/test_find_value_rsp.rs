use std::net::SocketAddr;
use crate::{
    Id, Network, NodeInfo,
    Value, ValueBuilder,
    dht::msg::{
        find_value_rsp::FindValueResponse,
        lookup_rsp::LookupResponse,
    }
};

fn make_node_info4() -> NodeInfo {
    let addr = format!("127.0.0.1:{}", 39001).parse::<SocketAddr>().unwrap();
    NodeInfo::new(Id::random(), addr)
}

fn make_node_info6() -> NodeInfo {
    let addr = format!("[::1]:{}", 39001).parse::<SocketAddr>().unwrap();
    NodeInfo::new(Id::random(), addr)
}

fn make_value() -> Value {
    let data = vec![1, 2, 3, 4, 5];
    ValueBuilder::new(&data)
        .build()
        .expect("Failed to build value")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_nodes() {
        let node4 = make_node_info4();
        let node6 = make_node_info6();

        let rsp = FindValueResponse::with_nodes(
            Some(vec![node4.clone()]),
            Some(vec![node6.clone()])
        );

        assert!(rsp.nodes4().is_some());
        assert!(rsp.nodes6().is_some());
        assert!(rsp.value().is_none());

        assert_eq!(rsp.value(), None);
        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        assert_eq!(rsp.nodes4(), Some([node4.clone()].as_slice()));
        assert_eq!(rsp.nodes6(), Some([node6.clone()].as_slice()));
        assert_eq!(rsp.nodes4(), rsp.nodes(Network::IPv4));
        assert_eq!(rsp.nodes6(), rsp.nodes(Network::IPv6));
    }

    #[test]
    fn test_with_value() {
        let value = make_value();
        let rsp = FindValueResponse::with_value(value.clone());

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.value().is_some());

        assert_eq!(rsp.value(), Some(&value));
    }

    #[test]
    fn test_serde_with_nodes() {
        let ni4 = make_node_info4();
        let ni6 = make_node_info6();
        let rsp = FindValueResponse::with_nodes(
            Some(vec![ni4.clone()]),
            Some(vec![ni6.clone()])
        );

        assert!(rsp.nodes4().is_some());
        assert!(rsp.nodes6().is_some());
        assert!(rsp.value().is_none());

        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        let encoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindValueResponse = serde_cbor::from_slice(encoded.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.token(), 0);
        assert!(decoded.nodes4().is_some());
        assert!(decoded.nodes6().is_some());
        assert!(decoded.value().is_none());

        let nodes4 = decoded.nodes4().unwrap();
        assert_eq!(nodes4.len(), 1);
        assert_eq!(nodes4[0], ni4);

        let nodes6 = decoded.nodes6().unwrap();
        assert_eq!(nodes6.len(), 1);
        assert_eq!(nodes6[0], ni6);
    }

    #[test]
    fn test_serde_with_value() {
        let value = make_value();
        let rsp = FindValueResponse::with_value(value.clone());

        assert!(rsp.nodes4().is_none());
        assert!(rsp.nodes6().is_none());
        assert!(rsp.value().is_some());

        assert_eq!(rsp.token(), 0);
        assert_eq!(rsp.value().unwrap(), &value);

        let encoded = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindValueResponse = serde_cbor::from_slice(encoded.as_slice())
            .expect("Deserialization failed");

        assert!(decoded.nodes4().is_none());
        assert!(decoded.nodes6().is_none());
        assert!(decoded.value().is_some());

        assert_eq!(decoded.token(), 0);
        assert_eq!(decoded.value().unwrap(), &value);
    }
}
