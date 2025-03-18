use std::rc::Rc;
use std::net::SocketAddr;
use crate::{
    Id,
    NodeInfo,
};

use crate::core::msg::{
    Msg,
    lookup_rsp::Msg as LookupMsg,
    find_node_rsp::Message,
};

#[test]
fn test_cbor() {
    let nodeid = Id::random();
    let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
    let ni = Rc::new(NodeInfo::new(nodeid.clone(), addr.clone()));

    let mut msg = Message::new();
    msg.populate_closest_nodes4(vec![ni.clone()]);

    let cval = msg.ser();
    let mut decoded_msg = Message::new();
    let result = decoded_msg.from_cbor(&cval);
    assert_eq!(result.is_some(), true);
    assert_eq!(decoded_msg.token(), 0);
    assert_eq!(decoded_msg.nodes4().is_some(), true);
    assert_eq!(decoded_msg.nodes6().is_some(), false);

    let nodes4 = decoded_msg.nodes4().unwrap();
    assert_eq!(nodes4.len(), 1);
    assert_eq!(nodes4[0], ni);
}

#[test]
fn test_cbor_with_ipv6() {
    let nodeid = Id::random();
    let addr = "127.0.0.1:29001".parse::<SocketAddr>().unwrap();
    let ni4 = Rc::new(NodeInfo::new(nodeid.clone(), addr.clone()));

    let nodeid = Id::random();
    let addr = "[::1]:29001".parse::<SocketAddr>().unwrap();
    let ni6 = Rc::new(NodeInfo::new(nodeid.clone(), addr.clone()));

    let mut msg = Message::new();
    msg.populate_closest_nodes4(vec![ni4.clone()]);
    msg.populate_closest_nodes6(vec![ni6.clone()]);

    let cval = msg.ser();
    let mut decoded_msg = Message::new();
    let result = decoded_msg.from_cbor(&cval);
    assert_eq!(result.is_some(), true);
    assert_eq!(decoded_msg.token(), 0);
    assert_eq!(decoded_msg.nodes4().is_some(), true);
    assert_eq!(decoded_msg.nodes6().is_some(), true);

    let nodes4 = decoded_msg.nodes4().unwrap();
    assert_eq!(nodes4.len(), 1);
    assert_eq!(nodes4[0], ni4);

    let nodes6 = decoded_msg.nodes6().unwrap();
    assert_eq!(nodes6.len(), 1);
    assert_eq!(nodes6[0], ni6);
}
