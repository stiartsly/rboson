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

use std::{
    str::FromStr,
    fmt, mem,
};
use bs58::decode;
use hex::FromHexError;
use static_assertions::const_assert;

use libsodium_sys::{
    crypto_sign_BYTES, crypto_sign_PUBLICKEYBYTES, crypto_sign_SECRETKEYBYTES,
    crypto_sign_SEEDBYTES, crypto_sign_detached, crypto_sign_ed25519_sk_to_pk,
    crypto_sign_final_create, crypto_sign_final_verify, crypto_sign_init, crypto_sign_keypair,
    crypto_sign_ed25519_sk_to_seed, crypto_sign_seed_keypair, crypto_sign_state,
    crypto_sign_update, crypto_sign_verify_detached,
    randombytes_buf,
};

use crate::{
    errors::{ArgumentError, CryptoError},
    Error, Result,
};
use crate::{as_uchar_ptr, as_uchar_ptr_mut};

const_assert!(PrivateKey::BYTES == crypto_sign_SECRETKEYBYTES as usize);
const_assert!(PublicKey::BYTES == crypto_sign_PUBLICKEYBYTES as usize);
const_assert!(KeyPair::SEED_BYTES == crypto_sign_SEEDBYTES as usize);
const_assert!(Signature::BYTES == crypto_sign_BYTES as usize);

#[derive(Debug, Clone, PartialEq, Eq)]
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
        self.0.fill(0);
    }

    /// Returns the 32-byte Ed25519 seed embedded in this private key.
    pub fn seed(&self) -> [u8; 32] {
        let mut seed = [0u8; 32];
        unsafe {
            crypto_sign_ed25519_sk_to_seed(as_uchar_ptr_mut!(seed), as_uchar_ptr!(self.0));
        }
        seed
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

        unsafe {
            // Always success
            crypto_sign_detached(
                as_uchar_ptr_mut!(signature),
                std::ptr::null_mut(),
                as_uchar_ptr!(data),
                data.len() as libc::c_ulonglong,
                as_uchar_ptr!(self.0),
            )
        };
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

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hexstr())?;
        Ok(())
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
        self.0.fill(0);
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

        let rc = unsafe {
            crypto_sign_verify_detached(
                as_uchar_ptr!(signature),
                as_uchar_ptr!(data),
                data.len() as libc::c_ulonglong,
                as_uchar_ptr!(self.0),
            )
        };
        Ok(rc == 0)
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
        let mut sk = [0u8; PrivateKey::BYTES];
        let mut pk = [0u8; PublicKey::BYTES];

        unsafe {
            // Always success
            crypto_sign_keypair(as_uchar_ptr_mut!(pk), as_uchar_ptr_mut!(sk));
        }

        KeyPair(PrivateKey(sk), PublicKey(pk))
    }

    pub fn random() -> Self {
        let mut seed = [0u8; KeyPair::SEED_BYTES];
        unsafe {
            randombytes_buf(seed.as_mut_ptr() as *mut libc::c_void, KeyPair::SEED_BYTES);
        }

        let mut sk = [0u8; PrivateKey::BYTES];
        let mut pk = [0u8; PublicKey::BYTES];

        unsafe {
            // Always success
            crypto_sign_seed_keypair(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr_mut!(sk),
                as_uchar_ptr!(seed),
            );
        }

        KeyPair(PrivateKey(sk), PublicKey(pk))
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

        let mut sk = [0u8; PrivateKey::BYTES];
        let mut pk = [0u8; PublicKey::BYTES];

        unsafe {
            // Always success
            crypto_sign_seed_keypair(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr_mut!(sk),
                as_uchar_ptr!(seed),
            );
        }

        Ok(KeyPair(PrivateKey(sk), PublicKey(pk)))
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

        let mut pk = [0u8; PublicKey::BYTES];
        unsafe {
            // Always success
            crypto_sign_ed25519_sk_to_pk(as_uchar_ptr_mut!(pk), as_uchar_ptr!(sk));
        }

        Ok(KeyPair(PrivateKey::try_from(sk).unwrap(), PublicKey(pk)))
    }
}

impl From<&PrivateKey> for KeyPair {
    fn from(sk: &PrivateKey) -> Self {
        let mut pk = [0u8; PublicKey::BYTES];

        unsafe {
            // Always success
            crypto_sign_ed25519_sk_to_pk(as_uchar_ptr_mut!(pk), as_uchar_ptr!(sk.as_bytes()));
        }
        KeyPair(sk.clone(), PublicKey(pk))
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

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct SignState([u8; std::mem::size_of::<crypto_sign_state>()]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    state: SignState,
}

impl Signature {
    pub const BYTES: usize = 64;

    pub fn new() -> Self {
        Self {
            state: SignState([0u8; std::mem::size_of::<crypto_sign_state>()]),
        }
    }

    pub fn reset(&mut self) -> &mut Self {
        assert!(
            mem::size_of::<SignState>() >= mem::size_of::<crypto_sign_state>(),
            "Inappropriate signature state size."
        );

        let s = &mut self.state.0 as *mut _ as *mut crypto_sign_state;
        unsafe {
            // Always success
            crypto_sign_init(s);
        }
        self
    }

    pub fn update(&mut self, part: &[u8]) -> &mut Self {
        let s = &mut self.state.0 as *mut _ as *mut crypto_sign_state;
        unsafe {
            // Always success
            crypto_sign_update(s, as_uchar_ptr!(part), part.len() as libc::c_ulonglong);
        }
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

        let s = &mut self.state.0 as *mut _ as *mut crypto_sign_state;
        unsafe {
            // Always success
            crypto_sign_final_create(
                s,
                as_uchar_ptr_mut!(signature),
                std::ptr::null_mut(),
                as_uchar_ptr!(sk.as_bytes()),
            );
        }
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

        let s = &mut self.state.0 as *mut _ as *mut crypto_sign_state;
        let rc = unsafe {
            crypto_sign_final_verify(s, as_uchar_ptr!(signature), as_uchar_ptr!(pk.as_bytes()))
        };

        Ok(rc == 0)
    }
}

impl Drop for Signature {
    fn drop(&mut self) {
        self.reset();
    }
}

pub fn sign(data: &[u8], sig: &mut [u8], sk: &PrivateKey) -> Result<usize> {
    sk.sign(data, sig)
}

pub fn sign_into(data: &[u8], sk: &PrivateKey) -> Result<Vec<u8>> {
    sk.sign_into(data)
}

pub fn verify(data: &[u8], signature: &[u8], pk: &PublicKey) -> Result<bool> {
    pk.verify(data, signature)
}
