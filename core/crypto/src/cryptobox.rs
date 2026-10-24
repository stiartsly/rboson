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

use libsodium_rs::{
    crypto_box as cbox,
    crypto_scalarmult::curve25519,
    crypto_sign as sign,
    utils,
};
use static_assertions::const_assert;
use std::fmt;

use crate::{
    errors::{ArgumentError, CryptoError},
    signature, Error, Result,
};

const_assert!(PrivateKey::BYTES == cbox::SECRETKEYBYTES);
const_assert!(PublicKey::BYTES == cbox::PUBLICKEYBYTES);
const_assert!(Nonce::BYTES == cbox::NONCEBYTES);
const_assert!(KeyPair::SEED_BYTES == 32);
const_assert!(CryptoBox::SYMMETRIC_KEY_BYTES == cbox::BEFORENMBYTES);
const_assert!(CryptoBox::MAC_BYTES == cbox::MACBYTES);
const_assert!(CryptoBox::SEAL_BYTES == cbox::SEALBYTES);

#[derive(Clone, PartialEq, Eq)]
pub struct PrivateKey([u8; Self::BYTES]);

impl PrivateKey {
    pub const BYTES: usize = 32;

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        crate::memzero(&mut self.0);
    }

    /// This key as a libsodium-rs key.
    fn raw(&self) -> cbox::SecretKey {
        cbox::SecretKey::from(self.0)
    }
}

impl TryFrom<&[u8]> for PrivateKey {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::BYTES {
            return Err(ArgumentError::new(format!(
                "Incorrect private key size {}, should be {}",
                bytes.len(),
                Self::BYTES
            )));
        }
        Ok(Self(bytes.try_into().unwrap()))
    }
}

impl TryFrom<&signature::PrivateKey> for PrivateKey {
    type Error = Error;
    fn try_from(sk: &signature::PrivateKey) -> Result<Self> {
        sign::ed25519_sk_to_curve25519(&sk.raw())
            .map(Self)
            .map_err(|_| CryptoError::new("converts Ed25519 key to x25519 key failed.").into())
    }
}

impl Drop for PrivateKey {
    fn drop(&mut self) {
        self.clear();
    }
}

impl AsRef<[u8]> for PrivateKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

// Private keys never print: as_bytes() gives one on purpose.
impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CryptoBox::PrivateKey(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey(pub(crate) [u8; Self::BYTES]);

impl PublicKey {
    pub const BYTES: usize = 32;

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        crate::memzero(&mut self.0);
    }

    /// This key as a libsodium-rs key.
    fn raw(&self) -> cbox::PublicKey {
        cbox::PublicKey::from(self.0)
    }
}

impl TryFrom<&[u8]> for PublicKey {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::BYTES {
            return Err(ArgumentError::new(format!(
                "Incorrect public key size {}, expected {}",
                bytes.len(),
                Self::BYTES
            )));
        }
        Ok(Self(bytes.try_into().unwrap()))
    }
}

impl TryFrom<&signature::PublicKey> for PublicKey {
    type Error = Error;
    fn try_from(pk: &signature::PublicKey) -> Result<Self> {
        sign::ed25519_pk_to_curve25519(&pk.raw())
            .map(Self)
            .map_err(|_| CryptoError::new("converts Ed25519 key to x25519 key failed.").into())
    }
}

impl Drop for PublicKey {
    fn drop(&mut self) {
        self.clear();
    }
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nonce([u8; Self::BYTES]);

impl Nonce {
    pub const BYTES: usize = 24;

    pub fn random() -> Self {
        Nonce(crate::random_array::<{ Self::BYTES }>())
    }

    pub fn generate() -> Self {
        Self::random()
    }

    pub fn increment(&mut self) -> &Self {
        // sodium_increment: little-endian, despite the wrapper's name.
        utils::increment_le(&mut self.0);
        self
    }

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        crate::memzero(&mut self.0)
    }

    /// This nonce as a libsodium-rs nonce.
    fn raw(&self) -> cbox::Nonce {
        cbox::Nonce::from_bytes_exact(self.0)
    }
}

impl TryFrom<&[u8]> for Nonce {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::BYTES {
            return Err(ArgumentError::new(format!(
                "Incorrect nonce key size {}, expected {}",
                bytes.len(),
                Self::BYTES
            )));
        }
        Ok(Self(bytes.try_into().unwrap()))
    }
}

impl Drop for Nonce {
    fn drop(&mut self) {
        self.clear();
    }
}

impl AsRef<[u8]> for Nonce {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl std::fmt::Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct KeyPair(PrivateKey, PublicKey);

impl KeyPair {
    pub const SEED_BYTES: usize = 32;

    pub fn new() -> Self {
        crate::ensure_init();
        let (pk, sk) = cbox::KeyPair::generate().into_tuple();
        Self(PrivateKey(sk.into()), PublicKey(pk.into()))
    }

    pub fn random() -> Self {
        let mut seed = crate::random_array::<{ Self::SEED_BYTES }>();
        let keypair = Self::try_from_seed(&seed).expect("X25519 key generation never fails");
        crate::memzero(&mut seed);
        keypair
    }

    pub fn generate() -> Self {
        Self::random()
    }

    pub fn try_from_seed(seed: &[u8]) -> Result<Self> {
        if seed.len() != KeyPair::SEED_BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid seed size {}, should be {}",
                seed.len(),
                KeyPair::SEED_BYTES
            )));
        }

        let (pk, sk) = cbox::KeyPair::from_seed(seed)
            .map_err(|e| CryptoError::new(format!("X25519 key generation failed: {e}")))?
            .into_tuple();
        Ok(Self(PrivateKey(sk.into()), PublicKey(pk.into())))
    }

    pub const fn private_key(&self) -> &PrivateKey {
        &self.0
    }

    pub fn to_private_key(&self) -> PrivateKey {
        self.0.clone()
    }

    pub const fn public_key(&self) -> &PublicKey {
        &self.1
    }

    pub fn to_public_key(&self) -> PublicKey {
        self.1.clone()
    }

    pub fn clear(&mut self) {
        self.0.clear();
        self.1.clear();
    }
}

impl TryFrom<&[u8]> for KeyPair {
    type Error = Error;
    fn try_from(sk: &[u8]) -> Result<Self> {
        if sk.len() != PrivateKey::BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid private key size {}, expected: {}",
                sk.len(),
                PrivateKey::BYTES
            )));
        }

        Ok(Self::from(&PrivateKey::try_from(sk)?))
    }
}

impl From<&PrivateKey> for KeyPair {
    fn from(sk: &PrivateKey) -> Self {
        let pk = curve25519::scalarmult_base(sk.as_bytes())
            .expect("X25519 public key derivation never fails");
        KeyPair(sk.clone(), PublicKey(pk))
    }
}

impl From<&signature::KeyPair> for KeyPair {
    fn from(kp: &signature::KeyPair) -> Self {
        let sk = PrivateKey::try_from(kp.private_key())
            .expect("Ed25519 to X25519 key conversion never fails");
        Self::from(&sk)
    }
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        self.clear();
    }
}

impl AsRef<PrivateKey> for KeyPair {
    fn as_ref(&self) -> &PrivateKey {
        &self.0
    }
}

impl AsRef<PublicKey> for KeyPair {
    fn as_ref(&self) -> &PublicKey {
        &self.1
    }
}

#[derive(Clone)]
pub struct CryptoBox([u8; Self::SYMMETRIC_KEY_BYTES]);

impl fmt::Debug for CryptoBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CryptoBox(<redacted>)")
    }
}

impl CryptoBox {
    pub const SYMMETRIC_KEY_BYTES: usize = 32;
    pub const MAC_BYTES: usize = 16;
    /// The bytes a sealed box adds to the message.
    pub const SEAL_BYTES: usize = 48;

    pub const fn size(&self) -> usize {
        Self::SYMMETRIC_KEY_BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        crate::memzero(&mut self.0)
    }

    pub fn encrypt(&self, plain: &[u8], cipher: &mut [u8], nonce: &Nonce) -> Result<usize> {
        let expected_len = plain.len() + CryptoBox::MAC_BYTES;
        if cipher.len() < expected_len {
            return Err(ArgumentError::new(format!(
                "The input buffer is insufficient."
            )));
        }

        let sealed = cbox::seal_afternm(plain, &nonce.raw(), &self.raw())
            .map_err(|_| CryptoError::new("Data encryption failed"))?;
        cipher[..expected_len].copy_from_slice(&sealed);
        Ok(expected_len)
    }

    pub fn decrypt(&self, cipher: &[u8], plain: &mut [u8], nonce: &Nonce) -> Result<usize> {
        let overhead = CryptoBox::MAC_BYTES;
        if cipher.len() < overhead {
            return Err(ArgumentError::new(format!(
                "Cipher text length {} is smaller than required overhead {}",
                cipher.len(),
                overhead
            )));
        }
        let expected_len = cipher.len() - overhead;
        if plain.len() < expected_len {
            return Err(ArgumentError::new(format!(
                "The input buffer is insufficient."
            )));
        }

        let opened = cbox::open_afternm(cipher, &nonce.raw(), &self.raw())
            .map_err(|_| CryptoError::new("Data decryption failed"))?;
        plain[..expected_len].copy_from_slice(&opened);
        Ok(expected_len)
    }

    /// Encrypts with an explicit nonce and returns raw ciphertext (no nonce prefix).
    pub fn encrypt_into(&self, plain: &[u8], nonce: &Nonce) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + Self::MAC_BYTES];
        self.encrypt(plain, &mut cipher, nonce).map(|_| cipher)
    }

    /// Decrypts raw ciphertext with an explicit nonce.
    pub fn decrypt_into(&self, cipher: &[u8], nonce: &Nonce) -> Result<Vec<u8>> {
        let mut plain = vec![0u8; cipher.len().saturating_sub(Self::MAC_BYTES)];
        self.decrypt(cipher, &mut plain, nonce).map(|_| plain)
    }
    
    /// Encrypts `message` anonymously for `receiver`, as a sealed box, into `cipher`: only the
    /// receiver can open it, and it does not tell who sent it.
    pub fn encrypt_sealed(message: &[u8], cipher: &mut [u8], receiver: &PublicKey) -> Result<usize> {
        let expected_len = message.len() + Self::SEAL_BYTES;
        if cipher.len() < expected_len {
            return Err(ArgumentError::new("The output buffer is insufficient."));
        }
        crate::ensure_init();
        let sealed = cbox::seal_box(message, &receiver.raw())
            .map_err(|_| CryptoError::new("Sealed-box encryption failed"))?;
        cipher[..expected_len].copy_from_slice(&sealed);
        Ok(expected_len)
    }

    /// Encrypts `message` anonymously for `receiver`, as a sealed box.
    pub fn encrypt_sealed_into(message: &[u8], receiver: &PublicKey) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; message.len() + Self::SEAL_BYTES];
        Self::encrypt_sealed(message, &mut cipher, receiver).map(|_| cipher)
    }

    /// Opens a sealed box with the receiver's public and private key, into `plain`.
    pub fn decrypt_sealed(
        cipher: &[u8],
        plain: &mut [u8],
        pk: &PublicKey,
        sk: &PrivateKey,
    ) -> Result<usize> {
        if cipher.len() < Self::SEAL_BYTES {
            return Err(CryptoError::new(format!(
                "Sealed box length {} is smaller than the required overhead {}",
                cipher.len(),
                Self::SEAL_BYTES
            )));
        }
        let expected_len = cipher.len() - Self::SEAL_BYTES;
        if plain.len() < expected_len {
            return Err(ArgumentError::new("The output buffer is insufficient."));
        }
        let opened = cbox::open_sealed_box(cipher, &pk.raw(), &sk.raw())
            .map_err(|_| CryptoError::new("Sealed-box decryption failed"))?;
        plain[..expected_len].copy_from_slice(&opened);
        Ok(expected_len)
    }

    /// Opens a sealed box with the receiver's public and private key.
    pub fn decrypt_sealed_into(cipher: &[u8], pk: &PublicKey, sk: &PrivateKey) -> Result<Vec<u8>> {
        let mut plain = vec![0u8; cipher.len().saturating_sub(Self::SEAL_BYTES)];
        Self::decrypt_sealed(cipher, &mut plain, pk, sk).map(|_| plain)
    }

    /// The shared key as a libsodium-rs key.
    fn raw(&self) -> cbox::PrecomputedKey {
        cbox::PrecomputedKey::from(self.0)
    }
}

impl TryFrom<(&PublicKey, &PrivateKey)> for CryptoBox {
    type Error = Error;
    fn try_from(kp: (&PublicKey, &PrivateKey)) -> Result<Self> {
        let key = cbox::beforenm(&kp.0.raw(), &kp.1.raw()).map_err(|_| {
            CryptoError::new("Compute symmetric key failed, wrong public key or private key")
        })?;
        Ok(CryptoBox(key.into()))
    }
}

impl Drop for CryptoBox {
    fn drop(&mut self) {
        self.clear();
    }
}

// ---------------------------------------------------------------------------
// Nonce-supplied variants (the nonce is carried separately by the caller)
// ---------------------------------------------------------------------------
pub fn encrypt(plain: &[u8], cipher: &mut [u8], nonce: &Nonce,
    pk: &PublicKey, sk: &PrivateKey) -> Result<usize> {

    let expected_len = plain.len() + CryptoBox::MAC_BYTES;
    if cipher.len() < expected_len {
        return Err(ArgumentError::new(format!(
            "The input buffer is insufficient."
        )));
    }

    let sealed = cbox::seal(plain, &nonce.raw(), &pk.raw(), &sk.raw())
        .map_err(|_| CryptoError::new("Data encryption failed"))?;
    cipher[..expected_len].copy_from_slice(&sealed);
    Ok(expected_len)
}

pub fn decrypt(cipher: &[u8], plain: &mut [u8], nonce: &Nonce,
        pk: &PublicKey, sk: &PrivateKey) -> Result<usize> {

    let overhead = CryptoBox::MAC_BYTES;
    if cipher.len() < overhead {
        return Err(ArgumentError::new(format!(
            "Cipher text length {} is smaller than required overhead {}",
            cipher.len(),
            overhead
        )));
    }
    let expected_len = cipher.len() - overhead;
    if plain.len() < expected_len {
        return Err(ArgumentError::new(format!(
            "The input buffer is insufficient."
        )));
    }

    let opened = cbox::open(cipher, &nonce.raw(), &pk.raw(), &sk.raw())
        .map_err(|_| CryptoError::new("Data decryption failed"))?;
    plain[..expected_len].copy_from_slice(&opened);
    Ok(expected_len)
}

/// Encrypts with an explicit nonce and returns raw ciphertext (no nonce prefix).
pub fn encrypt_into(
    plain: &[u8],
    nonce: &Nonce,
    pk: &PublicKey,
    sk: &PrivateKey,
) -> Result<Vec<u8>> {
    let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES];
    encrypt(plain, &mut cipher, nonce, pk, sk).map(|_| cipher)
}

/// Decrypts raw ciphertext with an explicit nonce.
pub fn decrypt_into(
    cipher: &[u8],
    nonce: &Nonce,
    pk: &PublicKey,
    sk: &PrivateKey,
) -> Result<Vec<u8>> {
    let mut plain = vec![0u8; cipher.len().saturating_sub(CryptoBox::MAC_BYTES)];
    decrypt(cipher, &mut plain, nonce, pk, sk).map(|_| plain)
}
