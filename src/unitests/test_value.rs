use crate::unitests::{
    create_random_bytes
};
use crate::{
    signature,
    cryptobox,
    Id
};
use crate::core::{
    value::PackBuilder
};

#[test]
fn test_pack_builder1() {
    let data = create_random_bytes(32);
    let val = PackBuilder::new(data.clone()).build();

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
    let val = PackBuilder::new(data.clone())
        .with_pk(Some(Id::from(keypair.to_public_key())))
        .build();

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
    let val = PackBuilder::new(data.clone())
        .with_pk(Some(Id::from(keypair.to_public_key())))
        .with_nonce(Some(nonce.clone()))
        .build();

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
fn test_pack_builder4() {
    let data = create_random_bytes(32);
    let keypair = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let sig = create_random_bytes(64);
    let val = PackBuilder::new(data.clone())
        .with_pk(Some(Id::from(keypair.to_public_key())))
        .with_nonce(Some(nonce.clone()))
        .with_sig(Some(sig.clone()))
        .with_sk(Some(keypair.private_key().clone()))
        .build();

    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), false);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.signature(), Some(sig.as_ref()));
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.public_key(), Some(&Id::from(keypair.to_public_key())));
    assert_eq!(val.private_key(), Some(keypair.private_key()));
    assert_eq!(val.nonce(), Some(nonce).as_ref());
}

#[test]
fn test_pack_builder5() {
    let data = create_random_bytes(32);
    let recipient = Id::random();
    let keypair = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let sig = create_random_bytes(64);
    let val = PackBuilder::new(data.clone())
        .with_pk(Some(Id::from(keypair.to_public_key())))
        .with_rec(Some(recipient.clone()))
        .with_nonce(Some(nonce.clone()))
        .with_sig(Some(sig.clone()))
        .with_sk(Some(keypair.private_key().clone()))
        .build();

    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), true);
    assert_eq!(val.is_valid(), false);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), true);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.signature(), Some(sig.as_ref()));
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.public_key(), Some(&Id::from(keypair.to_public_key())));
    assert_eq!(val.private_key(), Some(keypair.private_key()));
    assert_eq!(val.nonce(), Some(nonce).as_ref());
}
