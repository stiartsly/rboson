use crate::unitests::{
    create_random_bytes
};

use crate::{
    Id,
    PeerInfo
};

use crate::core::{
    peer_info::PackBuilder
};

#[test]
fn test_pack_builder() {
    let peerid = Id::random();
    let nodeid = Id::random();
    let origin = Id::random();

    let port  = 65535;
    let bytes = create_random_bytes(64);
    let url   = "https://testing.exmaple.com";
    let peer: PeerInfo = PackBuilder::new(nodeid.clone())
        .with_peerid(Some(peerid.clone()))
        .with_origin(Some(origin.clone()))
        .with_port(port)
        .with_url(Some(url.to_string()))
        .with_sig(Some(bytes.clone()))
        .build();

    assert_eq!(peer.id(), &peerid);
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &origin);
    assert_eq!(peer.has_private_key(), false);
    assert_eq!(peer.private_key().is_some(), false);
    assert_eq!(peer.private_key().is_none(), true);
    assert_eq!(peer.port(), port);
    assert_eq!(peer.has_alternative_url(), true);
    assert_eq!(peer.alternative_url().is_some(), true);
    assert_eq!(peer.alternative_url().is_none(), false);
    assert_eq!(peer.signature(), bytes);
    assert_eq!(peer.is_delegated(), true);
    assert_eq!(peer.is_valid(), false);
}
