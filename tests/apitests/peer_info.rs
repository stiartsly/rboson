use boson::{
    Id,
    PeerBuilder,
    signature,
};

#[test] //case1
fn test_new() {
    let nodeid = Id::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_port(65534)
        .build();

    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &nodeid);
    assert_eq!(peer.nodeid(), peer.origin());
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), false);
    assert_eq!(peer.alternative_url().is_none(), true);
    assert_eq!(peer.is_delegated(), false);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case2
fn test_with_keypair() {
    let nodeid = Id::random();
    let keypair = signature::KeyPair::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_keypair(&keypair)
        .with_port(65534)
        .build();

    assert_eq!(peer.id(), &Id::from(keypair.to_public_key()));
    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.private_key(), Some(keypair.private_key()));
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &nodeid);
    assert_eq!(peer.nodeid(), peer.origin());
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), false);
    assert_eq!(peer.alternative_url().is_some(), false);
    assert_eq!(peer.is_delegated(), false);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case3
fn test_with_proxy() {
    let nodeid = Id::random();
    let origin = Id::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_origin(&origin)
        .with_port(65534)
        .build();

    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &origin);
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), false);
    assert_eq!(peer.alternative_url().is_some(), false);
    assert_eq!(peer.is_delegated(), true);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case4
fn test_with_keypair_and_proxy() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let origin = Id::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_keypair(&keypair)
        .with_origin(&origin)
        .with_port(65534)
        .build();

    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.private_key(), Some(keypair.private_key()));
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &origin);
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), false);
    assert_eq!(peer.alternative_url().is_some(), false);
    assert_eq!(peer.is_delegated(), true);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case5
fn test_with_url() {
    let nodeid = Id::random();
    let url = "https://testing.example.com";
    let peer = PeerBuilder::new(&nodeid)
        .with_port(65534)
        .with_alternative_url(url)
        .build();

    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &nodeid);
    assert_eq!(peer.nodeid(), peer.origin());
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), true);
    assert_eq!(peer.alternative_url().is_some(), true);
    assert_eq!(peer.alternative_url(), Some(url));
    assert_eq!(peer.is_delegated(), false);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case6
fn test_with_keypair_and_url() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let url = "https://testing.example.com";
    let peer = PeerBuilder::new(&nodeid)
        .with_keypair(&keypair)
        .with_port(65534)
        .with_alternative_url(url)
        .build();

    assert_eq!(peer.id(), &Id::from(keypair.to_public_key()));
    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.private_key(), Some(keypair.private_key()));
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &nodeid);
    assert_eq!(peer.nodeid(), peer.origin());
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), true);
    assert_eq!(peer.alternative_url().is_some(), true);
    assert_eq!(peer.alternative_url(), Some(url));
    assert_eq!(peer.is_delegated(), false);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case7
fn test_with_proxy_and_url() {
    let nodeid = Id::random();
    let origin = Id::random();
    let url = "https://testing.example.com";
    let peer = PeerBuilder::new(&nodeid)
        .with_origin(&origin)
        .with_port(65534)
        .with_alternative_url(url)
        .build();

    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &origin);
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), true);
    assert_eq!(peer.alternative_url().is_some(), true);
    assert_eq!(peer.alternative_url(), Some(url));
    assert_eq!(peer.is_delegated(), true);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case8
fn test_with_keypair_proxy_and_url() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let origin = Id::random();
    let url = "https://testing.example.com";
    let peer = PeerBuilder::new(&nodeid)
        .with_keypair(&keypair)
        .with_origin(&origin)
        .with_port(65534)
        .with_alternative_url(url)
        .build();

    assert_eq!(peer.id(), &Id::from(keypair.to_public_key()));
    assert_eq!(peer.has_private_key(), true);
    assert_eq!(peer.private_key().is_some(), true);
    assert_eq!(peer.private_key(), Some(keypair.private_key()));
    assert_eq!(peer.nodeid(), &nodeid);
    assert_eq!(peer.origin(), &origin);
    assert_eq!(peer.port(), 65534);
    assert_eq!(peer.has_alternative_url(), true);
    assert_eq!(peer.alternative_url().is_some(), true);
    assert_eq!(peer.alternative_url(), Some(url));
    assert_eq!(peer.is_delegated(), true);
    assert_eq!(peer.signature().len(), 64);
    assert_eq!(peer.is_valid(), true);
}

#[test] // case9
fn test_equal() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let origin = Id::random();
    let url = "https://testing.example.com";

    let mut b1 = PeerBuilder::new(&nodeid);
    b1.with_keypair(&keypair)
        .with_origin(&origin)
        .with_port(65534)
        .with_alternative_url(url);

    let peer1 = b1.build();
    let peer2 = b1.build();

    let mut b3 = PeerBuilder::new(&nodeid);
    b3.with_port(65534);
    let peer3 = b3.build();

    assert_eq!(peer1.clone(), peer2);
    assert_ne!(peer1.clone(), peer3);
    assert_eq!(peer1.clone(), peer1);
}

#[test] // case10
fn test_peerid() {
    let keypair = signature::KeyPair::random();
    let origin = Id::random();
    let url1 = "https://testing1.example.com";

    let nodeid = Id::random();
    let peer1  = PeerBuilder::new(&nodeid)
        .with_keypair(&keypair)
        .with_origin(&origin)
        .with_port(65534)
        .with_alternative_url(url1)
        .build();

    let nodeid = Id::random();
    let peer2  = PeerBuilder::new(&nodeid)
        .with_keypair(&keypair)
        .with_origin(&origin)
        .with_port(65534)
        .build();

    assert_eq!(peer1.id(), peer2.id());
}
