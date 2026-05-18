use crate::core::{
    Id,
    signature,
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
    let id = Id::from(kp.public_key());
    let identity = CryptoIdentity::try_from(kp.private_key().as_bytes()).unwrap();
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
    let id = Id::from(kp.public_key());
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
    let identity1 = CryptoIdentity::try_from(kp1.private_key().as_bytes()).unwrap();
    let identity2 = CryptoIdentity::try_from(kp2.private_key().as_bytes()).unwrap();

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
fn test_from_private_key_bytes() {
    let kp = signature::KeyPair::random();
    let identity = CryptoIdentity::try_from(kp.private_key().as_bytes()).unwrap();

    assert_eq!(identity.id(), &Id::from(kp.public_key()));
    assert_eq!(identity.keypair().public_key(), kp.public_key());
    assert_eq!(
        identity.encryption_keypair().public_key().as_bytes(),
        crate::core::cryptobox::KeyPair::from(&kp).public_key().as_bytes()
    );
}

#[test]
fn test_from_private_key() {
    let kp = signature::KeyPair::random();
    let identity = CryptoIdentity::from(kp.private_key());
    assert_eq!(identity.id(), &Id::from(kp.public_key()));
    assert_eq!(identity.keypair().public_key(), kp.public_key());
}

#[test]
fn test_from_invalid_private_key() {
    let invalid_private_key = vec![0u8; 31];
    let result = CryptoIdentity::try_from(invalid_private_key.as_slice());
    assert!(result.is_err());
}

#[test]
fn test_verify_tampered_message() {
    let identity = CryptoIdentity::new();
    let data = b"Hello, World!";
    let tampered = b"Hello, Rust!";
    let result = identity.sign_into(data);
    assert!(result.is_ok());

    let sig = result.unwrap();
    let result = identity.verify(tampered, &sig);
    assert!(result.is_ok());

    let verified = result.unwrap();
    assert!(!verified);
}

#[test]
fn test_verify_message_signed_from_other_identity() {
    let signer = CryptoIdentity::new();
    let verifier = CryptoIdentity::new();
    let data = b"Hello, World!";
    let sig = signer.sign_into(data).unwrap();
    let result = verifier.verify(data, &sig);

    assert!(result.is_ok());
    let verified = result.unwrap();
    assert!(!verified);
}

#[test]
fn test_decrypt_data_with_wrong_sender() {
    let sender = CryptoIdentity::new();
    let receiver = CryptoIdentity::new();
    let other_sender = CryptoIdentity::new();
    let plain = b"Hello, World!";
    let result = sender.encrypt_into(receiver.id(), plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = receiver.decrypt_into(other_sender.id(), &cipher);
    assert!(result.is_err());
}

#[test]
fn test_decrypt_tampered_ciphertext() {
    let sender = CryptoIdentity::new();
    let receiver = CryptoIdentity::new();
    let plain = b"Hello, World!";
    let result = sender.encrypt_into(receiver.id(), plain);
    assert!(result.is_ok());

    let mut cipher = result.unwrap();
    let last = cipher.len() - 1;
    cipher[last] ^= 0x01;

    let result = receiver.decrypt_into(sender.id(), &cipher);
    assert!(result.is_err());
}
