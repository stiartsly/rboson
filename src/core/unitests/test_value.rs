use crate::core::{
    Id,
    Value,
    signature,
    cryptobox,
    unitests::create_random_bytes,
};

#[test]
fn test_pack_builder1() {
    let data = create_random_bytes(32);
    let val = Value::packed(None, None, None, None, data.clone(), 0);

    assert_eq!(val.is_mutable(), false);
    assert_eq!(val.is_signed(), false);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);

    assert_eq!(val.private_key().is_some(), false);
    assert_eq!(val.public_key().is_some(), false);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_none(), true);
    assert_eq!(val.nonce().is_none(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
}

#[test]
fn test_pack_builder2() {
    let data = create_random_bytes(32);
    let keypair = signature::KeyPair::random();
    let val = Value::packed(
        Some(Id::from(keypair.public_key())),
        None,
        None,
        None,
        data.clone(),
        0
    );

    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), false);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), false);
    assert_eq!(val.private_key().is_some(), false);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), false);
    assert_eq!(val.nonce().is_some(), false);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
}

#[test]
fn test_pack_builder3() {
    let data = create_random_bytes(32);
    let keypair = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let val = Value::packed(
        Some(Id::from(keypair.public_key())),
        None,
        Some(nonce.clone()),
        None,
        data.clone(),
        0
    );

    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), false);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), false);
    assert_eq!(val.private_key().is_some(), false);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.recipient().is_some(),false);
    assert_eq!(val.signature().is_some(), false);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.nonce(), Some(nonce).as_ref());
    assert_eq!(val.data(), &data);
}

#[test]
fn test_pack1() {
    let data = create_random_bytes(32);
    let keypair = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let sig = create_random_bytes(64);
    let val = Value::packed(
        Some(Id::from(keypair.public_key())),
        None,
        Some(nonce.clone()),
        Some(sig.clone()),
        data.clone(),
        0
    );

    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), false); // signature is random, so invalid
    assert_eq!(val.private_key().is_none(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.signature(), Some(sig.as_ref()));
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.public_key(), Some(&Id::from(keypair.public_key())));
    assert_eq!(val.private_key(), None);
    assert_eq!(val.nonce(), Some(nonce).as_ref());
}

#[test]
fn test_pack2() {
    let data = create_random_bytes(32);
    let recipient = Id::random();
    let keypair = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let sig = create_random_bytes(64);
    let val = Value::packed(
        Some(Id::from(keypair.public_key())),
        Some(recipient.clone()),
        Some(nonce.clone()),
        Some(sig.clone()),
        data.clone(),
        0
    );

    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), true);
    assert_eq!(val.is_valid(), false); // random sig
    assert_eq!(val.private_key().is_none(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), true);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.signature(), Some(sig.as_ref()));
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.public_key(), Some(&Id::from(keypair.public_key())));
    assert_eq!(val.private_key(), None);
    assert_eq!(val.nonce(), Some(nonce).as_ref());
}
