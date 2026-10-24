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

//! Ed25519 signatures.
//!
//! One-shot signing signs a whole message at once. It is what Boson signs on the wire, and what the
//! Java SDK does: [`PrivateKey::sign_into`] and [`PublicKey::verify`], or the functions [`sign`],
//! [`sign_into`] and [`verify`].
//!
//! Streaming signing, [`Signature`], signs or verifies a message given in parts. It uses libsodium's
//! multi-part API, which is Ed25519ph: it signs a SHA-512 hash of the message. Its signatures do not
//! verify with the one-shot functions or with the Java SDK, nor the other way round.

use std::{
    str::FromStr,
    fmt,
};
use bs58::decode;
use hex::FromHexError;
use static_assertions::const_assert;

use libsodium_rs::crypto_sign as sign;

use crate::{
    errors::{ArgumentError, CryptoError},
    Error, Result,
};

const_assert!(PrivateKey::BYTES == sign::SECRETKEYBYTES);
const_assert!(PublicKey::BYTES == sign::PUBLICKEYBYTES);
const_assert!(KeyPair::SEED_BYTES == sign::SEEDBYTES);
const_assert!(Signature::BYTES == sign::BYTES);

#[derive(Clone, PartialEq, Eq)]
pub struct PrivateKey([u8; Self::BYTES]);

impl PrivateKey {
    pub const BYTES: usize = 64;

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        crate::memzero(&mut self.0);
    }

    /// Returns the 32-byte Ed25519 seed embedded in this private key.
    pub fn seed(&self) -> [u8; 32] {
        sign::secret_key_to_seed(&self.raw()).expect("Ed25519 seed extraction never fails")
    }

    /// This key as a libsodium-rs key.
    pub(crate) fn raw(&self) -> sign::SecretKey {
        sign::SecretKey::from(self.0)
    }

    /// Creates a private key from an Ed25519 seed.
    pub fn from_seed(seed: &[u8]) -> Result<Self> {
        KeyPair::try_from_seed(seed).map(|keypair| keypair.to_private_key())
    }

    /// Derives a subkey with an 8-byte KDF context.
    pub fn derive(&self, sub_key_id: u64, context: &[u8]) -> Result<Self> {
        let seed = self.seed();
        let sub_seed = crate::key_derivation::derive_key(
            &seed,
            sub_key_id,
            context,
            KeyPair::SEED_BYTES,
        )?;
        Self::from_seed(&sub_seed)
    }

    /// Derives a subkey from a string context.
    pub fn derive_with_context(&self, sub_key_id: u64, context: &str) -> Result<Self> {
        let context = crate::key_derivation::context_bytes(context)?;
        self.derive(sub_key_id, &context)
    }

    pub fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        if signature.len() != Signature::BYTES {
            return Err(CryptoError::new(format!(
                "Incorrect signature length {}, expected {}",
                signature.len(),
                Signature::BYTES
            )));
        }

        let sig = sign::sign_detached(data, &self.raw())
            .map_err(|e| CryptoError::new(format!("Signing failed: {e}")))?;
        signature.copy_from_slice(&sig);
        Ok(Signature::BYTES)
    }

    pub fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut sig = vec![0u8; Signature::BYTES];
        self.sign(data, sig.as_mut()).map(|_| sig)
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(self.0)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .into_string()
    }

    pub fn to_hexstr(&self) -> String {
        format!("0x{}", hex::encode(self.0))
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
        Ok(PrivateKey(bytes.try_into().unwrap()))
    }
}

impl TryFrom<&str> for PrivateKey {
    type Error = Error;
    fn try_from(input: &str) -> Result<Self> {
        let mut bytes = vec![0u8; Self::BYTES];
        if input.starts_with("0x") {
            hex::decode_to_slice(&input[2..], &mut bytes[..]).map_err(|e| match e {
                FromHexError::InvalidHexCharacter { c, index } => {
                    ArgumentError::new(format!("Invalid hex character {} at position {}", c, index))
                }
                FromHexError::OddLength => {
                    ArgumentError::new(format!("Odd hex string length {}", input.len()))
                }
                FromHexError::InvalidStringLength => {
                    ArgumentError::new(format!("Invalid hex string length"))
                }
            })?;
            return Ok(PrivateKey(bytes.try_into().unwrap()));
        }

        // Decode as base58 if it doesn't start with "0x"
        let len = bs58::decode(input)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .onto(&mut bytes[..])
            .map_err(|e| match e {
                decode::Error::BufferTooSmall => {
                    ArgumentError::new(format!("Invalid base58 string length"))
                }
                decode::Error::InvalidCharacter { character, index } => ArgumentError::new(
                    format!("Invalid base58 character {} at {}", character, index),
                ),
                _ => ArgumentError::new(format!("Invalid base58 with unknown error")),
            })?;
        if len != Self::BYTES {
            return Err(ArgumentError::new(format!(
                "Incorrect private key size {len}, should be {}",
                Self::BYTES
            )));
        }
        Ok(PrivateKey(bytes.try_into().unwrap()))
    }
}

impl FromStr for PrivateKey {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
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

// Private keys never print: to_hexstr() and to_base58() show one on purpose.
impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Signature::PrivateKey(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey(pub(crate) [u8; Self::BYTES]);

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
        Ok(PublicKey(bytes.try_into().unwrap()))
    }
}

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
    pub(crate) fn raw(&self) -> sign::PublicKey {
        sign::PublicKey::from(self.0)
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(self.0)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .into_string()
    }

    pub fn to_hexstr(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    pub fn try_from_hexstr(input: &str) -> Result<Self> {
        let input = input.strip_prefix("0x").unwrap_or(input);
        let mut bytes = [0u8; Self::BYTES];
        hex::decode_to_slice(input, &mut bytes[..]).map_err(|e| match e {
            FromHexError::InvalidHexCharacter { c, index } => {
                ArgumentError::new(format!("Invalid hex character {} at position {}", c, index))
            }
            FromHexError::OddLength => {
                ArgumentError::new(format!("Odd hex string length {}", input.len()))
            }
            FromHexError::InvalidStringLength => {
                ArgumentError::new(format!("Invalid hex string length"))
            }
        })?;
        Ok(PublicKey(bytes))
    }

    pub fn try_from_base58(input: &str) -> Result<Self> {
        let mut bytes = [0u8; Self::BYTES];
        let len = bs58::decode(input)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .onto(&mut bytes[..])
            .map_err(|e| match e {
                decode::Error::BufferTooSmall => {
                    ArgumentError::new(format!("Invalid base58 string length"))
                }
                decode::Error::InvalidCharacter { character, index } => ArgumentError::new(
                    format!("Invalid base58 character {} at {}", character, index),
                ),
                _ => ArgumentError::new(format!("Invalid base58 with unknown error")),
            })?;
        if len != Self::BYTES {
            return Err(ArgumentError::new(format!(
                "Incorrect public key size {len}, should be {}",
                Self::BYTES
            )));
        }
        Ok(PublicKey(bytes))
    }

    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        if signature.len() != Signature::BYTES {
            return Err(CryptoError::new(format!(
                "Incorrect signature length {}, should be {}",
                signature.len(),
                Signature::BYTES
            )));
        }

        let signature: &[u8; Signature::BYTES] = signature.try_into().unwrap();
        Ok(sign::verify_detached(signature, data, &self.raw()))
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

impl FromStr for PublicKey {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        if s.starts_with("0x") {
            Self::try_from_hexstr(s)
        } else {
            Self::try_from_base58(s)
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeyPair(PrivateKey, PublicKey);

impl KeyPair {
    pub const SEED_BYTES: usize = 32;

    pub fn new() -> Self {
        crate::ensure_init();
        let (pk, sk) = sign::KeyPair::generate()
            .expect("Ed25519 key generation never fails")
            .into_tuple();
        KeyPair(PrivateKey(sk.into()), PublicKey(pk.into()))
    }

    pub fn random() -> Self {
        let mut seed = crate::random_array::<{ KeyPair::SEED_BYTES }>();
        let keypair = Self::try_from_seed(&seed).expect("Ed25519 key generation never fails");
        crate::memzero(&mut seed);
        keypair
    }

    pub fn generate() -> Self {
        Self::random()
    }

    pub fn try_from_seed<'a>(seed: &[u8]) -> Result<Self> {
        if seed.len() != KeyPair::SEED_BYTES {
            return Err(ArgumentError::new(format!(
                "Incorrect seed buffer size {}, expected {}",
                seed.len(),
                KeyPair::SEED_BYTES
            )));
        }

        let (pk, sk) = sign::KeyPair::from_seed(seed)
            .map_err(|e| CryptoError::new(format!("Ed25519 key generation failed: {e}")))?
            .into_tuple();
        Ok(KeyPair(PrivateKey(sk.into()), PublicKey(pk.into())))
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
                "Incorrect private key size {}, expected {}",
                sk.len(),
                PrivateKey::BYTES
            )));
        }

        Ok(KeyPair::from(&PrivateKey::try_from(sk)?))
    }
}

impl From<&PrivateKey> for KeyPair {
    fn from(sk: &PrivateKey) -> Self {
        let pk = sign::PublicKey::from_secret_key(&sk.raw())
            .expect("Ed25519 public key extraction never fails");
        KeyPair(sk.clone(), PublicKey(pk.into()))
    }
}

impl From<PrivateKey> for KeyPair {
    fn from(sk: PrivateKey) -> Self {
        Self::from(&sk)
    }
}

impl FromStr for KeyPair {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let sk = PrivateKey::try_from(s)?;
        Ok(KeyPair::from(sk))
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

/// A streaming Ed25519ph signer and verifier, for a message given in parts. Its signatures are not
/// interchangeable with one-shot signatures; see the module documentation.
#[derive(Debug, Clone)]
pub struct Signature {
    state: sign::State,
}

impl Signature {
    pub const BYTES: usize = 64;

    pub fn new() -> Self {
        Self {
            state: sign::State::new().expect("Ed25519ph state initialization never fails"),
        }
    }

    pub fn reset(&mut self) -> &mut Self {
        self.state = sign::State::new().expect("Ed25519ph state initialization never fails");
        self
    }

    pub fn update(&mut self, part: &[u8]) -> &mut Self {
        self.state.update(part).expect("Ed25519ph update never fails");
        self
    }

    pub fn sign(&mut self, signature: &mut [u8], sk: &PrivateKey) -> Result<usize> {
        if signature.len() != Signature::BYTES {
            return Err(CryptoError::new(format!(
                "Incorrect signature length {}, should be {}",
                signature.len(),
                Signature::BYTES
            )));
        }

        let sig = self
            .state
            .finalize_create(&sk.raw())
            .map_err(|e| CryptoError::new(format!("Signing failed: {e}")))?;
        signature.copy_from_slice(&sig);
        Ok(Signature::BYTES)
    }

    pub fn sign_into(&mut self, sk: &PrivateKey) -> Result<Vec<u8>> {
        let mut sig = vec![0u8; Self::BYTES];
        self.sign(sig.as_mut(), sk).map(|_| sig)
    }

    pub fn verify(&mut self, signature: &[u8], pk: &PublicKey) -> Result<bool> {
        if signature.len() != Signature::BYTES {
            return Err(CryptoError::new(format!(
                "Incorrect signature length {}, should be {}",
                signature.len(),
                Signature::BYTES
            )));
        }

        let signature: &[u8; Signature::BYTES] = signature.try_into().unwrap();
        Ok(self.state.finalize_verify(signature, &pk.raw()))
    }
}

/// Signs `data` in one shot, into `sig`.
pub fn sign(data: &[u8], sig: &mut [u8], sk: &PrivateKey) -> Result<usize> {
    sk.sign(data, sig)
}

/// Signs `data` in one shot.
pub fn sign_into(data: &[u8], sk: &PrivateKey) -> Result<Vec<u8>> {
    sk.sign_into(data)
}

/// Verifies a one-shot signature of `data`.
pub fn verify(data: &[u8], signature: &[u8], pk: &PublicKey) -> Result<bool> {
    pk.verify(data, signature)
}
