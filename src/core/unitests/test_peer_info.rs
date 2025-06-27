use crate::core::{
    Id,
    PeerInfo,
    PeerBuilder,
    peer_info::PackBuilder,
    unitests::create_random_bytes,
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

#[test]
fn test_from_cbor() {
    let nodeid = Id::random();
    let port = 65535;
    let url = "https:://testing.example.com";
    let peer = PeerBuilder::new(&nodeid)
        .with_port(port)
        .with_alternative_url(Some(url))
        .build();

    let cbor = peer.to_cbor();
    let result = PeerInfo::from_cbor(&cbor);
    assert_eq!(result.is_some(), true);

    let parsed = result.unwrap();
    assert_eq!(peer.nodeid(), parsed.nodeid());
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.id(), parsed.id());
    assert_eq!(peer.port(), parsed.port());
    assert_eq!(peer.port(), port);
    assert_eq!(parsed.has_alternative_url(), true);
    assert_eq!(peer.alternative_url(), parsed.alternative_url());
}
