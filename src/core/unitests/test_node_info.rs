use std::net::{
    IpAddr,
    Ipv4Addr,
    Ipv6Addr,
    SocketAddr
};

use crate::core::{
    Id,
    NodeInfo
};

/*
 package APIs for testcases
 - from_cbor
 - to_cbor
 */

#[test]
fn test_from_cbor1() {
    let id = Id::random();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
    let ni = NodeInfo::new(id.clone(), addr);
    let cbor = ni.to_cbor();
    let result = NodeInfo::from_cbor(&cbor);
    assert_eq!(result.is_some(), true);

    let ni_from = result.unwrap();
    assert_eq!(ni_from, ni);
    assert_eq!(ni_from.id(), &id);
    assert_eq!(ni_from.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    assert_eq!(ni_from.port(), 12345);
    assert_eq!(ni_from.socket_addr(), &addr);
    assert_eq!(ni_from.version(), 0);
    assert_eq!(ni_from.is_ipv4(), true);
    assert_eq!(ni_from.is_ipv6(), false);
}

#[test]
fn test_from_cbor2() {
    let id = Id::random();
    let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 12345);
    let ni = NodeInfo::new(id.clone(), addr);
    // ni.set_version(55);
    let cbor = ni.to_cbor();
    let result = NodeInfo::from_cbor(&cbor);
    assert_eq!(result.is_some(), true);

    let ni_from = result.unwrap();
    assert_eq!(ni_from, ni);
    assert_eq!(ni_from.id(), &id);
    assert_eq!(ni_from.ip(), IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));
    assert_eq!(ni_from.port(), 12345);
    assert_eq!(ni_from.socket_addr(), &addr);
   // assert_eq!(ni_from.version(), 55);
    assert_eq!(ni_from.is_ipv4(), false);
    assert_eq!(ni_from.is_ipv6(), true);
}
