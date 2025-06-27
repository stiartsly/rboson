use crate::{
    signature,
    Id,
    PeerBuilder,
};

use crate::dht::msg::{
    msg::Msg,
    find_peer_rsp::Message,
};

#[test]
fn test_cbor() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let origin = Id::random();
    let peer = PeerBuilder::new(&nodeid)
        .with_keypair(Some(&keypair))
        .with_origin(Some(&origin))
        .with_port(65534)
        .build();

    let mut msg = Message::new();
    msg.populate_peers(vec![peer]);

    let cval = msg.ser();
    let mut decoded_msg = Message::new();
    let result = decoded_msg.from_cbor(&cval);
    assert_eq!(result.is_some(), true);
    assert_eq!(msg.peers().len(), 1);
    assert_eq!(decoded_msg.peers().len(), 1);

    let peer = &msg.peers()[0];
    let decoded_peer = &decoded_msg.peers()[0];
    assert_eq!(peer.origin(), &origin);
    assert_eq!(decoded_peer.id(), peer.id());
    assert_eq!(decoded_peer.nodeid(), peer.nodeid());
    assert_eq!(decoded_peer.origin(), &origin);
    assert_eq!(decoded_peer.alternative_url().is_some(), false);
    assert_eq!(decoded_peer.alternative_url(), peer.alternative_url());
    assert_eq!(decoded_peer.signature(), peer.signature());
}

#[test]
fn test_cbor_with_url() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let origin = Id::random();
    let url = "https://testing.example.com";
    let peer = PeerBuilder::new(&nodeid)
        .with_keypair(Some(&keypair))
        .with_origin(Some(&origin))
        .with_port(65534)
        .with_alternative_url(Some(url))
        .build();

    let mut msg = Message::new();
    msg.populate_peers(vec![peer]);

    let cval = msg.ser();
    let mut decoded_msg = Message::new();
    let result = decoded_msg.from_cbor(&cval);
    assert_eq!(result.is_some(), true);
    assert_eq!(msg.peers().len(), 1);
    assert_eq!(decoded_msg.peers().len(), 1);

    let decoded_peer = &decoded_msg.peers()[0];
    assert_eq!(decoded_peer.alternative_url().is_some(), true);
    assert_eq!(decoded_peer.alternative_url(), Some(url));
}

#[test]
fn test_cbor_with_more_peers() {
    let keypair = signature::KeyPair::random();
    let nodeid = Id::random();
    let origin = Id::random();
    let peer1 = PeerBuilder::new(&nodeid)
        .with_keypair(Some(&keypair))
        .with_origin(Some(&origin))
        .with_port(65534)
        .build();

    let nodeid = Id::random();
    let url = "https://testing2.example.com";
    let peer2 = PeerBuilder::new(&nodeid)
        .with_keypair(Some(&keypair))
        .with_origin(Some(&origin))
        .with_port(65534)
        .with_alternative_url(Some(url))
        .build();

    let mut msg = Message::new();
    msg.populate_peers(vec![peer1, peer2]);

    let cval = msg.ser();
    let mut decoded_msg = Message::new();
    let result = decoded_msg.from_cbor(&cval);
    assert_eq!(result.is_some(), true);
    assert_eq!(msg.peers().len(), 2);
    assert_eq!(decoded_msg.peers().len(), 2);
}
