use boson::{
    signature,
    cryptobox,
    Id,
    Value,
    ValueBuilder,
    SignedBuilder,
    EncryptedBuilder
};
use crate::create_random_bytes;

/* Value methods:
 - id(),
 - public_key()
 - private_key()
 - recipient()
 - sequence_number()
 - nonce()
 - signature()
 - data()
 - size()
 - is_encrypted()
 - is_signed()
 - is_mutable()
 - is_valid()
 */

/** ValueBuilder methods
- new(data)
- trait::TryInto
 */
#[test]
fn test_immutable() {
    let data = create_random_bytes(32);
    let rc = ValueBuilder::new(&data).build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), false);
    assert_eq!(val.is_signed(), false);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), false);
    assert_eq!(val.private_key().is_some(), false);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), false);
    assert_eq!(val.nonce().is_some(), false);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

/** SignedBuilder methods.
 - new(data)
 - trait::TryInto
 */
#[test]
fn test_signed_simple() {
    let data = create_random_bytes(32);
    let rc = SignedBuilder::new(&data).build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_signed_with_keypair() {
    let data = create_random_bytes(32);
    let kp = signature::KeyPair::random();
    let rc = SignedBuilder::new(&data)
        .with_keypair(&kp)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_eq!(val.data(), &data);
    assert_eq!(val.public_key(), Some(&kp.to_public_key().into()));
    assert_eq!(val.private_key(), Some(kp.private_key()));
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_signed_with_nonce() {
    let data = create_random_bytes(32);
    let nonce = cryptobox::Nonce::random();
    let rc = SignedBuilder::new(&data)
        .with_sequence_number(55)
        .with_nonce(&nonce)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 55);
    assert_eq!(val.data(), &data);
    assert_eq!(val.nonce(), Some(nonce).as_ref());
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_signed_with_seq() {
    let data = create_random_bytes(32);
    let rc = SignedBuilder::new(&data)
        .with_sequence_number(55)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), 55);
    assert_eq!(val.data(), &data);
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_signed_full() {
    let data = create_random_bytes(32);
    let kp = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let seq = 55;
    let rc = SignedBuilder::new(&data)
        .with_keypair(&kp)
        .with_sequence_number(seq)
        .with_nonce(&nonce)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), false);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), false);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.sequence_number(), seq);
    assert_eq!(val.data(), &data);
    assert_eq!(val.public_key(), Some(&kp.to_public_key().into()));
    assert_eq!(val.private_key(), Some(kp.private_key()));
    assert_eq!(val.nonce(), Some(nonce).as_ref());
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_encrypted_simple() {
    let data = create_random_bytes(32);
    let kp = signature::KeyPair::random();
    let rec: Id = kp.to_public_key().into();
    let rc = EncryptedBuilder::new(&data, &rec).build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), true);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_ne!(val.data(), &data);
    assert_eq!(val.recipient(), Some(rec).as_ref());
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_encrypted_with_keypair() {
    let data = create_random_bytes(32);
    let kp = signature::KeyPair::random();
    let rec: Id = signature::KeyPair::random()
        .to_public_key()
        .into();
    let rc = EncryptedBuilder::new(&data, &rec)
        .with_keypair(&kp)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), true);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_ne!(val.data(), &data);
    assert_eq!(val.recipient(), Some(rec).as_ref());
    assert_eq!(val.public_key(), Some(&kp.to_public_key().into()));
    assert_eq!(val.private_key(), Some(kp.private_key()));
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_encrypted_with_nonce() {
    let data = create_random_bytes(32);
    let nonce = cryptobox::Nonce::random();
    let rec: Id = signature::KeyPair::random()
        .to_public_key()
        .into();
    let rc = EncryptedBuilder::new(&data, &rec)
        .with_nonce(&nonce)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), true);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.sequence_number(), 0);
    assert_ne!(val.data(), &data);
    assert_eq!(val.recipient(), Some(rec).as_ref());
    assert_eq!(val.nonce(), Some(&nonce));
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}

#[test]
fn test_encrypted_with_full() {
    let data = create_random_bytes(32);
    let kp = signature::KeyPair::random();
    let nonce = cryptobox::Nonce::random();
    let rec: Id = signature::KeyPair::random()
        .to_public_key()
        .into();
    let rc = EncryptedBuilder::new(&data, &rec)
        .with_keypair(&kp)
        .with_nonce(&nonce)
        .with_sequence_number(55)
        .build();
    assert_eq!(rc.is_ok(), true);

    let val: Value = rc.unwrap();
    assert_eq!(val.is_mutable(), true);
    assert_eq!(val.is_signed(), true);
    assert_eq!(val.is_encrypted(), true);
    assert_eq!(val.is_valid(), true);
    assert_eq!(val.public_key().is_some(), true);
    assert_eq!(val.private_key().is_some(), true);
    assert_eq!(val.recipient().is_some(), true);
    assert_eq!(val.nonce().is_some(), true);
    assert_eq!(val.signature().is_some(), true);
    assert_eq!(val.sequence_number(), 55);
    assert_ne!(val.data(), &data);
    assert_eq!(val.recipient(), Some(rec).as_ref());
    assert_eq!(val.nonce(), Some(&nonce));
    assert_eq!(val.public_key(), Some(&kp.to_public_key().into()));
    assert_eq!(val.private_key(), Some(kp.private_key()));
    assert_eq!(val.id(), <Value as Into<Id>>::into(val));
}
