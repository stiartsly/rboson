use crate::core::{
    Id,
    PeerInfo,
    PeerBuilder,
    signature::KeyPair,
    unitests::create_random_bytes,
};

#[test]
fn test_peer_builder() {
    let endpoint = "https://example.com:8080";
    let kp = KeyPair::random();

    let peer = PeerBuilder::new(endpoint)
        .with_key(kp.clone())
        .build()
        .expect("Failed to build peer info");

    assert_eq!(peer.endpoint(), endpoint);
    assert_eq!(peer.id(), &Id::from(kp.public_key()));
    assert_eq!(peer.has_private_key(), true);
    assert!(peer.is_valid());
    assert!(!peer.is_authenticated()); // No node associated
}

#[test]
fn test_packed() {
    let pk = Id::random();
    let nonce = create_random_bytes(24);
    let seq = 1;
    let nodeid = Some(Id::random());
    let node_sig = Some(create_random_bytes(64));
    let sig = create_random_bytes(64);
    let fingerprint = 12345;
    let endpoint = "tcp://1.2.3.4:9000".to_string();
    let extra = Some(vec![1, 2, 3]);

    let peer = PeerInfo::packed(
        pk.clone(),
        nonce.clone(),
        seq,
        nodeid.clone(),
        node_sig.clone(),
        sig.clone(),
        fingerprint,
        endpoint.clone(),
        extra.clone()
    );

    assert_eq!(peer.id(), &pk);
    assert_eq!(peer.nonce(), &nonce);
    assert_eq!(peer.sequence_number(), seq);
    assert_eq!(peer.nodeid(), nodeid.as_ref());
    assert_eq!(peer.node_signature(), node_sig.as_deref());
    assert_eq!(peer.signature(), &sig);
    assert_eq!(peer.fingerprint(), fingerprint);
    assert_eq!(peer.endpoint(), endpoint);
    assert_eq!(peer.extra_data(), extra.as_deref());

    assert!(!peer.has_private_key());
}

#[test]
#[ignore] // TODO:
fn test_serde() {
    let endpoint = "https://example.com:8080";
    let kp = KeyPair::random();
    let peer = PeerBuilder::new(endpoint)
        .with_key(kp.clone())
        .build()
        .expect("Failed to build peer info");

    let cbor = serde_cbor::to_vec(&peer).expect("Failed to serialize PeerInfo");
    let deserialized: PeerInfo = serde_cbor::from_slice(&cbor).expect("Failed to deserialize PeerInfo");

    assert_eq!(peer.id(), deserialized.id());
    assert_eq!(peer.endpoint(), deserialized.endpoint());
    assert_eq!(peer.nonce(), deserialized.nonce());
    assert_eq!(peer.signature(), deserialized.signature());
    assert_eq!(deserialized.has_private_key(), false);

    assert!(deserialized.is_valid());
}
