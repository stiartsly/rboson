use std::net::{
    IpAddr,
    Ipv4Addr,
    SocketAddr
};

use crate::core::{
    Id,
    NodeInfo
};

#[test]
fn test_serde() {
    let id = Id::random();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
    let ni = NodeInfo::new(id.clone(), addr);
    let cbor = serde_cbor::to_vec(&ni).expect("Failed to serialize NodeInfo");
    let ni_from: NodeInfo = serde_cbor::from_slice(&cbor).expect("Failed to deserialize NodeInfo");
    assert_eq!(ni_from, ni);
    assert_eq!(ni_from.id(), &id);
}
