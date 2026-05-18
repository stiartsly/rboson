use std::net::SocketAddr;

use crate::{
    Id,
    NodeInfo,
    Value,
};
use crate::dht::msg::{
    find_value_rsp::FindValueResponse,
    lookup_rsp::LookupResponse,
};

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
