use crate::core::{
    id::Id,
    cryptobox::{self, CryptoBox, Nonce},
    signature,
    CryptoContext,
};
/*
 Testcases for critical methods:
 - new(..)
 - from_private_key(..)
 */

#[test]
fn test_from_private_key() {
    let sig_kp1 = signature::KeyPair::random();
    let sig_kp2 = signature::KeyPair::random();
    let box_kp1 = cryptobox::KeyPair::from(&sig_kp1);
    let box_kp2 = cryptobox::KeyPair::from(&sig_kp2);

    let id1 = Id::from(sig_kp1.to_public_key());
    let id2 = Id::from(sig_kp2.to_public_key());

    let ctx1 = CryptoContext::from_private_key(&id2, &box_kp1.private_key());
    let mut ctx2 = CryptoContext::from_private_key(&id1, &box_kp2.private_key());

    assert_eq!(&id2, ctx1.id());
    assert_eq!(&id1, ctx2.id());

    // testing encrypt_into and decrypt_into methods
    let plain = "Hello, World!".as_bytes();
    let result = ctx2.encrypt_into(plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = ctx1.decrypt_into(&cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());

    // testing encrypt and decrypt methods
    let plain = "Hello, World!".as_bytes();
    let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
    let result = ctx2.encrypt(plain, &mut cipher);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), cipher.len());

    let result = ctx1.decrypt_into(&cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());

    let mut decrypted = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
    let result = ctx1.decrypt(&cipher, &mut decrypted);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), decrypted.len());
    assert_eq!(plain, decrypted.as_slice());
}

#[test]
fn test_from_cryptobox() {
    let sig_kp1 = signature::KeyPair::random();
    let sig_kp2 = signature::KeyPair::random();
    let box_kp1 = cryptobox::KeyPair::from(&sig_kp1);
    let box_kp2 = cryptobox::KeyPair::from(&sig_kp2);

    let id1 = Id::from(sig_kp1.to_public_key());
    let id2 = Id::from(sig_kp2.to_public_key());

    let box1 = cryptobox::CryptoBox::try_from((box_kp2.public_key(), box_kp1.private_key())).unwrap();
    let box2 = cryptobox::CryptoBox::try_from((box_kp1.public_key(), box_kp2.private_key())).unwrap();
    let ctx1 = CryptoContext::new(&id2, box1);
    let mut ctx2 = CryptoContext::new(&id1, box2);

    assert_eq!(&id2, ctx1.id());
    assert_eq!(&id1, ctx2.id());

    // testing encrypt_into and decrypt_into methods
    let plain = "Hello, World!".as_bytes();
    let result = ctx2.encrypt_into(plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = ctx1.decrypt_into(&cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());

    // testing encrypt and decrypt methods
    let plain = "Hello, World!".as_bytes();
    let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
    let result = ctx2.encrypt(plain, &mut cipher);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), cipher.len());

    let result = ctx1.decrypt_into(&cipher);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(plain, decrypted.as_slice());

    let mut decrypted = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
    let result = ctx1.decrypt(&cipher, &mut decrypted);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), decrypted.len());
    assert_eq!(plain, decrypted.as_slice());
}
