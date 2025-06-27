use crate::core::{
    Id,
    signature,
    Identity,
    CryptoIdentity,
};
/*
 Testcases for critical methods:
 - test_sign(..)
 - test_encryption(..)
 */

#[test]
fn test_sign1() {
    let identity = CryptoIdentity::new();

    let data = "Hello, World!".as_bytes();
    let result = identity.sign_into(data);
    assert!(result.is_ok());

    let signature = result.unwrap();
    let result = identity.verify(data, &signature);
    assert!(result.is_ok());
}

#[test]
fn test_sign2() {
    let kp = signature::KeyPair::random();
    let id = Id::from(kp.to_public_key());
    let identity = CryptoIdentity::from_private_key(kp.private_key());
    assert_eq!(identity.id(), &id);

    let data = "Hello, World!".as_bytes();
    let result = identity.sign_into(data);
    assert!(result.is_ok());

    let signature = result.unwrap();
    let result = identity.verify(data, &signature);
    assert!(result.is_ok());
}

#[test]
fn test_sign3() {
    let kp = signature::KeyPair::random();
    let id = Id::from(kp.to_public_key());
    let identity = CryptoIdentity::from_keypair(kp);
    assert_eq!(identity.id(), &id);

    let data = "Hello, World!".as_bytes();
    let result = identity.sign_into(data);
    assert!(result.is_ok());

    let signature = result.unwrap();
    let result = identity.verify(data, &signature);
    assert!(result.is_ok());
}

#[test]
fn test_encryption1() {
    let identity1 = CryptoIdentity::new();
    let identity2 = CryptoIdentity::new();

    let plain = "Hello, World!".as_bytes();
    let result = identity1.encrypt_into(identity2.id(), plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = identity2.decrypt_into(identity1.id(), &cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());
}

#[test]
fn test_encryption2() {
    let kp1 = signature::KeyPair::random();
    let kp2 = signature::KeyPair::random();
    let identity1 = CryptoIdentity::from_keypair(kp1);
    let identity2 = CryptoIdentity::from_keypair(kp2);

    let plain = "Hello, World!".as_bytes();
    let result = identity1.encrypt_into(identity2.id(), plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = identity2.decrypt_into(identity1.id(), &cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());
}

#[test]
fn test_encryption3() {
    let kp1 = signature::KeyPair::random();
    let kp2 = signature::KeyPair::random();
    let identity1 = CryptoIdentity::from_private_key(kp1.private_key());
    let identity2 = CryptoIdentity::from_private_key(kp2.private_key());

    let plain = "Hello, World!".as_bytes();
    let result = identity1.encrypt_into(identity2.id(), plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = identity2.decrypt_into(identity1.id(), &cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());
}
