// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::{
    cryptobox::{CryptoBox, Nonce},
    signature, CryptoIdentity, Id, Identity,
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
fn test_sign() {
    // Test CryptoIdentity::from with a valid key pair.
    let kp = signature::KeyPair::random();
    let id = Id::from(kp.public_key());
    let identity = CryptoIdentity::from(kp);
    assert_eq!(identity.id(), &id);

    let data = "Hello, World!".as_bytes();
    let result = identity.sign_into(data);
    assert!(result.is_ok());

    let signature = result.unwrap();
    let result = identity.verify(data, &signature);
    assert!(result.is_ok());

    // test CryptoIdentity::try_from with valid private key bytes
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
fn test_encrypt1() {
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
fn test_encrypt2() {
    let kp1 = signature::KeyPair::random();
    let kp2 = signature::KeyPair::random();
    let identity1 = CryptoIdentity::from(kp1);
    let identity2 = CryptoIdentity::from(kp2);

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
fn test_from_sk_bytes() {
    let kp = signature::KeyPair::random();
    let identity = CryptoIdentity::try_from(kp.private_key().as_bytes()).unwrap();

    assert_eq!(identity.id(), &Id::from(kp.public_key()));
    assert_eq!(identity.signature_key().public_key(), kp.public_key());
    assert_eq!(
        identity.encryption_key().public_key().as_bytes(),
        crate::cryptobox::KeyPair::from(&kp)
            .public_key()
            .as_bytes()
    );
}

#[test]
fn test_from_private_key() {
    let kp = signature::KeyPair::random();
    let identity = CryptoIdentity::from(signature::KeyPair::from(kp.private_key().clone()));
    assert_eq!(identity.id(), &Id::from(kp.public_key()));
    assert_eq!(identity.signature_key().public_key(), kp.public_key());
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
    let sender  = CryptoIdentity::new();
    let recver  = CryptoIdentity::new();
    let other   = CryptoIdentity::new();

    let plain   = b"Hello, World!";
    let result  = sender.encrypt_into(recver.id(), plain);
    assert!(result.is_ok());

    let cipher = result.unwrap();
    let result = recver.decrypt_into(other.id(), &cipher);
    assert!(result.is_err());
}

#[test]
fn test_decrypt_tampered_ciphertext() {
    let sender = CryptoIdentity::new();
    let recver = CryptoIdentity::new();
    let plain = b"Hello, World!";
    let result = sender.encrypt_into(recver.id(), plain);
    assert!(result.is_ok());

    let mut cipher = result.unwrap();
    let last = cipher.len() - 1;
    cipher[last] ^= 0x01;

    let result = recver.decrypt_into(sender.id(), &cipher);
    assert!(result.is_err());
}

#[test]
fn test_encrypt_prepends_nonce() {
    let sender = CryptoIdentity::new();
    let recver = CryptoIdentity::new();
    let plain = b"Hello, explicit nonce!";

    let cipher = sender.encrypt_into(recver.id(), plain).unwrap();
    assert_eq!(cipher.len(), plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES);
    assert_eq!(recver.decrypt_into(sender.id(), &cipher).unwrap(), plain);
}

#[test]
fn test_encrypt_with_nonce_is_raw_ciphertext() {
    let sender = CryptoIdentity::new();
    let recver = CryptoIdentity::new();
    let nonce = Nonce::random();
    let plain = b"Hello, raw ciphertext!";

    let cipher = sender
        .encrypt_with_nonce_into(recver.id(), nonce.as_bytes(), plain)
        .unwrap();
    assert_eq!(cipher.len(), plain.len() + CryptoBox::MAC_BYTES);
    assert_eq!(
        recver
            .decrypt_with_nonce_into(sender.id(), nonce.as_bytes(), &cipher)
            .unwrap(),
        plain
    );

    let wrong_nonce = Nonce::random();
    assert!(recver
        .decrypt_with_nonce_into(sender.id(), wrong_nonce.as_bytes(), &cipher)
        .is_err());
}

#[test]
fn test_decrypt_short_ciphertext() {
    let sender = CryptoIdentity::new();
    let recver = CryptoIdentity::new();
    // Short ciphertexts must return error rather than panic
    assert!(recver.decrypt_into(sender.id(), &[]).is_err());
    assert!(recver.decrypt_into(sender.id(), &[0u8; 10]).is_err());
    assert!(recver.decrypt_into(sender.id(), &[0u8; 39]).is_err());

    let mut out = [0u8; 10];
    assert!(recver.decrypt(sender.id(), &[0u8; 20], &mut out).is_err());
}
