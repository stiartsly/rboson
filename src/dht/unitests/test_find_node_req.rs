use crate::Id;
use crate::dht::msg::{
    lookup_req::LookupRequest,
    find_node_req::FindNodeRequest,
};

#[test]
fn test_serde() {
    let nodeid = Id::random();
    let req = FindNodeRequest::new(
        nodeid.clone(),
        true,
        false,
        true,
    );

    let cbor = serde_cbor::to_vec(&req).expect("Serialization failed");
    let decoded: FindNodeRequest = serde_cbor::from_slice(&cbor)
        .expect("Deserialization failed");

    assert_eq!(decoded.target(), &nodeid);
    assert_eq!(decoded.want4(), true);
    assert_eq!(decoded.want6(), false);
    assert_eq!(decoded.want_token(), true);
}
