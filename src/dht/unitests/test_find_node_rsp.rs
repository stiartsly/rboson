use std::net::SocketAddr;
use crate::{Id, NodeInfo};
use crate::dht::msg::{
    lookup_rsp::LookupResponse,
    find_node_rsp::FindNodeResponse,
};

#[test]
fn test_serde() {
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
