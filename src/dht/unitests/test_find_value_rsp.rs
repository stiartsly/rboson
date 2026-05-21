use std::net::SocketAddr;

use crate::{
    Id,
    NodeInfo,
    Value,
    dht::msg::{
        find_value_rsp::FindValueResponse,
        lookup_rsp::LookupResponse,
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

        let rsp = FindValueResponse::new(
            Some(vec![node.clone()]),
            None,
        );

        assert_eq!(rsp.nodes4(), Some([node.clone()].as_slice()));
        assert_eq!(rsp.nodes6(), None);
        assert_eq!(rsp.has_value(), false);
        assert_eq!(rsp.value(), None);
    }

    #[test]
    fn test_rsp_with_value() {
        let value = Value::packed(None, None, None, None, vec![1, 2, 3], 0);
        let rsp = FindValueResponse::from(value.clone());

        assert_eq!(rsp.nodes4(), None);
        assert_eq!(rsp.nodes6(), None);
        assert_eq!(rsp.has_value(), true);
        assert_eq!(rsp.value(), Some(&value));
    }

    #[test]
    fn test_display_with_nodes() {
        let node4 = NodeInfo::new(
            Id::random(),
            "127.0.0.1:29001".parse::<SocketAddr>().unwrap(),
        );
        let node6 = NodeInfo::new(
            Id::random(),
            "[::1]:29001".parse::<SocketAddr>().unwrap(),
        );

        let rsp = FindValueResponse::new(
            Some(vec![node4.clone()]),
            Some(vec![node6.clone()]),
        );

        let str = format!("{}", rsp);
        assert!(str.contains("n4:"));
        assert!(str.contains(&format!("[{}]", node4)));
        assert!(str.contains("n6:"));
        assert!(str.contains(&format!("[{}]", node6)));
    }

    #[test]
    fn test_display_with_value() {
        let value = Value::packed(None, None, None, None, vec![1, 2, 3], 0);
        let rsp = FindValueResponse::from(value.clone());

        let str = format!("{}", rsp);
        assert_eq!(str, format!("v:[{}]", value));
    }

    #[test]
    fn test_serde_with_nodes() {
        let nodeid = Id::random();
        let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
        let ni4 = NodeInfo::new(nodeid.clone(), addr);

        let nodeid = Id::random();
        let addr = "[::1]:29001".parse::<SocketAddr>().unwrap();
        let ni6 = NodeInfo::new(nodeid.clone(), addr);

        let rsp = FindValueResponse::new(
            Some(vec![ni4.clone()]),
            Some(vec![ni6.clone()])
        );

        assert_eq!(rsp.nodes4().is_some(), true);
        assert_eq!(rsp.nodes4().unwrap().len(), 1);
        assert_eq!(rsp.nodes6().is_some(), true);
        assert_eq!(rsp.nodes6().unwrap().len(), 1);

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindValueResponse = serde_cbor::from_slice(cbor.as_slice())
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
    fn test_serde_with_value() {
        let data = vec![1, 2, 3, 4, 5];
        let value = Value::packed(None, None, None, None, data.clone(), 0);

        let rsp = FindValueResponse::from(value.clone());

        assert_eq!(rsp.nodes4().is_none(), true);
        assert_eq!(rsp.nodes6().is_none(), true);
        assert_eq!(rsp.token(), 0);

        assert_eq!(rsp.has_value(), true);
        assert_eq!(rsp.value().is_some(), true);
        assert_eq!(rsp.value().unwrap(), &value);

        let cbor = serde_cbor::to_vec(&rsp)
            .expect("Serialization failed");
        let decoded: FindValueResponse = serde_cbor::from_slice(cbor.as_slice())
            .expect("Deserialization failed");

        assert_eq!(decoded.nodes4().is_none(), true);
        assert_eq!(decoded.nodes6().is_none(), true);
        assert_eq!(decoded.token(), 0);

        assert_eq!(decoded.has_value(), true);
        assert_eq!(decoded.value().is_some(), true);
        assert_eq!(decoded.value().unwrap(), &value);
    }
}
