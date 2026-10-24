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

// Ported from Boson.Native, adapted to this crate's API, plus sealed boxes.

use crate::{
    cryptobox::{self, CryptoBox, KeyPair, Nonce},
    signature,
};

#[test]
fn precomputed_roundtrip() {
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let nonce = Nonce::random();

    let a2b = CryptoBox::try_from((bob.public_key(), alice.private_key())).unwrap();
    let b2a = CryptoBox::try_from((alice.public_key(), bob.private_key())).unwrap();

    let cipher = a2b.encrypt_into(b"hello bob", &nonce).unwrap();
    assert_eq!(b2a.decrypt_into(&cipher, &nonce).unwrap(), b"hello bob");
    let zero = Nonce::try_from([0u8; Nonce::BYTES].as_slice()).unwrap();
    assert!(b2a.decrypt_into(&cipher, &zero).is_err());
    assert_eq!(a2b.as_bytes().len(), CryptoBox::SYMMETRIC_KEY_BYTES);
}

#[test]
fn one_shot_matches_precomputed() {
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let nonce = Nonce::random();

    let cipher = cryptobox::encrypt_into(b"msg", &nonce, bob.public_key(), alice.private_key()).unwrap();
    let plain = cryptobox::decrypt_into(&cipher, &nonce, alice.public_key(), bob.private_key()).unwrap();
    assert_eq!(plain, b"msg");

    let b2a = CryptoBox::try_from((alice.public_key(), bob.private_key())).unwrap();
    assert_eq!(b2a.decrypt_into(&cipher, &nonce).unwrap(), b"msg");
}

#[test]
fn sealed_box_roundtrip() {
    let bob = KeyPair::random();
    let cipher = CryptoBox::encrypt_sealed_into(b"anon", bob.public_key()).unwrap();
    assert_eq!(cipher.len(), b"anon".len() + CryptoBox::SEAL_BYTES);
    let plain = CryptoBox::decrypt_sealed_into(&cipher, bob.public_key(), bob.private_key()).unwrap();
    assert_eq!(plain, b"anon");

    let eve = KeyPair::random();
    assert!(CryptoBox::decrypt_sealed_into(&cipher, eve.public_key(), eve.private_key()).is_err());
    assert!(CryptoBox::decrypt_sealed_into(&cipher[..10], bob.public_key(), bob.private_key()).is_err());
}

#[test]
fn sealed_box_into_buffers() {
    let bob = KeyPair::random();
    let mut cipher = [0u8; 4 + CryptoBox::SEAL_BYTES];
    assert_eq!(CryptoBox::encrypt_sealed(b"anon", &mut cipher, bob.public_key()).unwrap(), cipher.len());
    let mut plain = [0u8; 4];
    assert_eq!(CryptoBox::decrypt_sealed(&cipher, &mut plain, bob.public_key(), bob.private_key()).unwrap(), 4);
    assert_eq!(&plain, b"anon");

    let mut short = [0u8; 10];
    assert!(CryptoBox::encrypt_sealed(b"anon", &mut short, bob.public_key()).is_err());
    assert!(CryptoBox::decrypt_sealed(&cipher, &mut [0u8; 3], bob.public_key(), bob.private_key()).is_err());
}

#[test]
fn secrets_are_redacted() {
    let kp = KeyPair::random();
    assert_eq!(format!("{}", kp.private_key()), "<redacted>");
    assert_eq!(format!("{:?}", kp.private_key()), "CryptoBox::PrivateKey(<redacted>)");
    assert!(format!("{:?}", kp).contains("CryptoBox::PrivateKey(<redacted>)"));
    let shared = CryptoBox::try_from((kp.public_key(), kp.private_key())).unwrap();
    assert_eq!(format!("{:?}", shared), "CryptoBox(<redacted>)");
}

#[test]
fn ed25519_to_curve25519_agrees_both_sides() {
    // Encryption between two identities using keys converted from Ed25519 must round-trip,
    // exercising both the public and the private key conversion.
    let alice = KeyPair::from(&signature::KeyPair::random());
    let bob = KeyPair::from(&signature::KeyPair::random());
    let nonce = Nonce::random();

    let cipher = cryptobox::encrypt_into(b"x25519", &nonce, bob.public_key(), alice.private_key()).unwrap();
    let plain = cryptobox::decrypt_into(&cipher, &nonce, alice.public_key(), bob.private_key()).unwrap();
    assert_eq!(plain, b"x25519");
}

#[test]
fn nonce_increment_is_little_endian() {
    let mut n = Nonce::try_from([0u8; Nonce::BYTES].as_slice()).unwrap();
    n.increment();
    let mut expected = [0u8; Nonce::BYTES];
    expected[0] = 1;
    assert_eq!(n.as_bytes(), &expected);

    let mut wrap = [0u8; Nonce::BYTES];
    wrap[0] = 0xff;
    let mut n2 = Nonce::try_from(wrap.as_slice()).unwrap();
    n2.increment();
    let mut expected2 = [0u8; Nonce::BYTES];
    expected2[1] = 1;
    assert_eq!(n2.as_bytes(), &expected2);
}

#[test]
fn public_key_derivation_matches() {
    let kp = KeyPair::random();
    let kp2 = KeyPair::try_from(kp.private_key().as_bytes()).unwrap();
    assert_eq!(kp.public_key(), kp2.public_key());
}
