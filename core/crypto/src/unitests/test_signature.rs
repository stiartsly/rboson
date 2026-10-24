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

// Ported from Boson.Native, adapted to this crate's API.

use crate::signature::{self, KeyPair, PrivateKey, Signature};

#[test]
fn sign_and_verify_roundtrip() {
    let kp = KeyPair::random();
    let msg = b"boson signature test";
    let sig = kp.private_key().sign_into(msg).unwrap();
    assert!(kp.public_key().verify(msg, &sig).unwrap());
    assert!(!kp.public_key().verify(b"tampered", &sig).unwrap());
    assert!(signature::verify(msg, &signature::sign_into(msg, kp.private_key()).unwrap(), kp.public_key()).unwrap());
}

#[test]
fn private_key_is_seed_then_public_key() {
    let kp = KeyPair::random();
    let sk = kp.private_key().as_bytes();
    assert_eq!(sk.len(), PrivateKey::BYTES);
    // libsodium private keys carry the public key in the trailing 32 bytes.
    assert_eq!(&sk[32..], kp.public_key().as_bytes());
    assert_eq!(&sk[..32], kp.private_key().seed());
}

#[test]
fn from_seed_is_deterministic() {
    let seed = [7u8; KeyPair::SEED_BYTES];
    let a = KeyPair::try_from_seed(&seed).unwrap();
    let b = KeyPair::try_from_seed(&seed).unwrap();
    assert_eq!(a.public_key(), b.public_key());
    assert_eq!(a.private_key(), b.private_key());
}

#[test]
fn keypair_from_private_key_recovers_public() {
    let kp = KeyPair::random();
    let kp2 = KeyPair::try_from(kp.private_key().as_bytes()).unwrap();
    assert_eq!(kp.public_key(), kp2.public_key());
    assert_eq!(KeyPair::from(kp.private_key()).public_key(), kp.public_key());
}

#[test]
fn wrong_length_signature_fails_closed() {
    let kp = KeyPair::random();
    assert!(!kp.public_key().verify(b"x", &[0u8; 10]).unwrap_or(false));
}

#[test]
fn derive_is_deterministic_and_context_sensitive() {
    let kp = KeyPair::random();
    let a = kp.private_key().derive_with_context(1, "boson").unwrap();
    let b = kp.private_key().derive_with_context(1, "boson").unwrap();
    let c = kp.private_key().derive_with_context(2, "boson").unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn signature_matches_the_key_operations() {
    let kp = KeyPair::random();
    let sig = Signature::sign_into(b"message", kp.private_key()).unwrap();
    assert_eq!(sig.len(), Signature::BYTES);
    assert_eq!(sig, kp.private_key().sign_into(b"message").unwrap());

    let mut buffer = [0u8; Signature::BYTES];
    assert_eq!(Signature::sign(b"message", &mut buffer, kp.private_key()).unwrap(), Signature::BYTES);
    assert_eq!(buffer.as_slice(), sig.as_slice());

    assert!(Signature::verify(b"message", &sig, kp.public_key()).unwrap());
    assert!(!Signature::verify(b"tampered", &sig, kp.public_key()).unwrap());
    assert!(Signature::verify(b"message", &sig[..10], kp.public_key()).is_err());
}

#[test]
fn private_keys_are_redacted() {
    let kp = KeyPair::random();
    assert_eq!(format!("{}", kp.private_key()), "<redacted>");
    assert_eq!(format!("{:?}", kp.private_key()), "Signature::PrivateKey(<redacted>)");
    assert!(format!("{:?}", kp).contains("Signature::PrivateKey(<redacted>)"));
    assert!(kp.private_key().to_hexstr().starts_with("0x"));
}
