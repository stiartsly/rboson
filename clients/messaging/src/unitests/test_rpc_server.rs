use super::*;

fn identity() -> CryptoIdentity {
    CryptoIdentity::from(signature::KeyPair::random())
}

#[test]
fn seal_protects_for_the_recipient_and_envelopes_for_the_service() {
    let (user, device, service, friend) = (identity(), identity(), identity(), identity());
    let outbound = Outbound {
        user_identity: &user,
        device_identity: &device,
        peer_id: service.id(),
    };

    let (message, payload) = outbound
        .seal(friend.id(), HANDSHAKE_MESSAGE, 42, b"hello".to_vec(), Some(friend.id()), "handshake")
        .unwrap();
    assert_eq!(message.version, MESSAGE_VERSION);
    assert_eq!(message.message_type, HANDSHAKE_MESSAGE);
    assert_eq!(message.id, RpcServer::message_id(device.id(), 42).unwrap());

    // The service opens the envelope with the device key...
    let envelope = decrypt_payload(&service, device.id(), &payload, "envelope").unwrap();
    let opened: PhotonMessage = decode_cbor_bytes(&envelope, "envelope").unwrap();
    assert_eq!(opened.recipient, *friend.id());
    // ...and only the friend can open the body.
    let body = binary_payload(opened.payload, "Handshake").unwrap();
    assert_eq!(decrypt_payload(&friend, user.id(), &body, "handshake").unwrap(), b"hello");
    assert!(decrypt_payload(&service, user.id(), &body, "handshake").is_err());
}

#[test]
fn seal_leaves_the_body_alone_without_an_inner_layer() {
    let (user, device, service) = (identity(), identity(), identity());
    let outbound = Outbound {
        user_identity: &user,
        device_identity: &device,
        peer_id: service.id(),
    };
    let (message, _) = outbound
        .seal(service.id(), CONTROL_MESSAGE, 7, b"rpc".to_vec(), None, "RPC")
        .unwrap();
    assert_eq!(message.payload, Value::Bytes(b"rpc".to_vec()));
    assert!(outbound.seal(service.id(), CONTROL_MESSAGE, -1, vec![], None, "RPC").is_err());
}

#[test]
fn binary_payload_rejects_other_values() {
    assert_eq!(binary_payload(Value::Bytes(vec![1]), "X").unwrap(), vec![1]);
    let error = binary_payload(Value::Null, "Notification").unwrap_err();
    assert!(error.to_string().contains("Notification payload is not binary"));
}
