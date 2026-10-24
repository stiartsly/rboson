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

use super::{
    cryptobox::{self, CryptoBox, Nonce},
    signature::{self, Signature},
    CryptoContext,
    Id,
    identity::Identity,
    errors::{ArgumentError, Result},
};
use std::hash;

#[derive(Clone)]
pub struct CryptoIdentity {
    id: Id,
    signature_key: signature::KeyPair,
    encryption_key: cryptobox::KeyPair,
}

impl CryptoIdentity {
    pub const OVERHEAD_BASE: usize = CryptoBox::MAC_BYTES;
    pub const OVERHEAD_PADDED_NONCE: usize = Self::OVERHEAD_BASE + Nonce::BYTES;

    fn from_keypair(keypair: signature::KeyPair) -> CryptoIdentity {
        Self {
            id              : Id::from(keypair.public_key()),
            encryption_key  : cryptobox::KeyPair::from(&keypair),
            signature_key   : keypair,
        }
    }

    pub fn new() -> Self {
        Self::from_keypair(signature::KeyPair::random())
    }

    pub fn random() -> Self {
        Self::from_keypair(signature::KeyPair::random())
    }

    pub fn signature_key(&self) -> &signature::KeyPair {
        &self.signature_key
    }

    pub fn encryption_key(&self) -> &cryptobox::KeyPair {
        &self.encryption_key
    }
}

impl CryptoIdentity {
    fn crypto_box_for(&self, id: &Id) -> Result<CryptoBox> {
        CryptoBox::try_from((&id.to_encryption_key()?, self.encryption_key.private_key()))
    }
}

impl Default for CryptoIdentity {
    fn default() -> Self {
        Self::random()
    }
}

impl Eq for CryptoIdentity {}

impl PartialEq for CryptoIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl hash::Hash for CryptoIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl TryFrom<&[u8]> for CryptoIdentity {
    type Error = super::Error;

    fn try_from(private_key: &[u8]) -> Result<Self> {
        signature::KeyPair::try_from(private_key).map(Self::from_keypair)
    }
}

impl From<signature::KeyPair> for CryptoIdentity {
    fn from(keypair: signature::KeyPair) -> Self {
        Self::from_keypair(keypair)
    }
}

impl From<signature::PrivateKey> for CryptoIdentity {
    fn from(private_key: signature::PrivateKey) -> Self {
        Self::from_keypair(signature::KeyPair::from(private_key))
    }
}

impl AsRef<CryptoIdentity> for CryptoIdentity {
    fn as_ref(&self) -> &CryptoIdentity {
        self
    }
}

impl Drop for CryptoIdentity {
    fn drop(&mut self) {
        self.signature_key.clear();
        self.encryption_key.clear()
    }
}

impl Identity for CryptoIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], sig: &mut [u8]) -> Result<usize> {
        signature::sign(data, sig, self.signature_key.private_key())
    }

    fn verify(&self, data: &[u8], sig: &[u8]) -> Result<bool> {
        signature::verify(data, sig, self.signature_key.public_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; Signature::BYTES];
        self.sign(data, &mut v).map(|_| v)
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        if cipher.len() < plain.len() + Self::OVERHEAD_PADDED_NONCE {
            return Err(ArgumentError::new("The output buffer is insufficient."));
        }
        let nonce = Nonce::random();
        cipher[..Nonce::BYTES].copy_from_slice(nonce.as_bytes());

        let written = self.encrypt_with_nonce(
            recipient,
            nonce.as_bytes(),
            plain,
            &mut cipher[Nonce::BYTES..],
        )?;
        Ok(Nonce::BYTES + written)
    }

    fn encrypt_into(&self, rec: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + Self::OVERHEAD_PADDED_NONCE];
        self.encrypt(rec, plain, &mut cipher).map(|_| cipher)
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        if cipher.len() <= Self::OVERHEAD_PADDED_NONCE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} must be longer than the overhead {}",
                cipher.len(),
                Self::OVERHEAD_PADDED_NONCE
            )));
        }
        if plain.len() < cipher.len() - Self::OVERHEAD_PADDED_NONCE {
            return Err(ArgumentError::new(format!(
                "Plaintext buffer length {} is smaller than the required {}",
                plain.len(),
                cipher.len() - Self::OVERHEAD_PADDED_NONCE
            )));
        }
        let nonce = &cipher[..Nonce::BYTES];
        self.decrypt_with_nonce(sender, nonce, &cipher[Nonce::BYTES..], plain)
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        if cipher.len() <= Self::OVERHEAD_PADDED_NONCE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} must be longer than the overhead {}",
                cipher.len(),
                Self::OVERHEAD_PADDED_NONCE
            )));
        }
        let mut plain = vec![0u8; cipher.len() - Self::OVERHEAD_PADDED_NONCE];
        self.decrypt(sender, cipher, &mut plain).map(|_| plain)
    }

    fn encrypt_with_nonce(
        &self,
        recipient: &Id,
        nonce: &[u8],
        plain: &[u8],
        cipher: &mut [u8],
    ) -> Result<usize> {
        if cipher.len() < plain.len() + Self::OVERHEAD_BASE {
            return Err(ArgumentError::new("The output buffer is insufficient."));
        }

        let nonce = Nonce::try_from(nonce)?;
        self.crypto_box_for(recipient)?.encrypt(plain, cipher, &nonce)
    }

    fn decrypt_with_nonce(
        &self,
        sender: &Id,
        nonce: &[u8],
        cipher: &[u8],
        plain: &mut [u8],
    ) -> Result<usize> {
        if cipher.len() <= Self::OVERHEAD_BASE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} must be longer than the overhead {}",
                cipher.len(),
                Self::OVERHEAD_BASE
            )));
        }
        if plain.len() < cipher.len() - Self::OVERHEAD_BASE {
            return Err(ArgumentError::new(format!(
                "Plaintext buffer length {} is smaller than the required {}",
                plain.len(),
                cipher.len() - Self::OVERHEAD_BASE
            )));
        }
        let nonce = Nonce::try_from(nonce)?;
        self.crypto_box_for(sender)?.decrypt(cipher, plain, &nonce)
    }

    fn encrypt_with_nonce_into(
        &self,
        rec: &Id,
        nonce: &[u8],
        plain: &[u8],
    ) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + Self::OVERHEAD_BASE];
        self.encrypt_with_nonce(rec, nonce, plain, &mut cipher)
            .map(|_| cipher)
    }

    fn decrypt_with_nonce_into(
        &self,
        sender: &Id,
        nonce: &[u8],
        cipher: &[u8],
    ) -> Result<Vec<u8>> {
        if cipher.len() <= Self::OVERHEAD_BASE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} must be longer than the overhead {}",
                cipher.len(),
                Self::OVERHEAD_BASE
            )));
        }
        let mut plain = vec![0u8; cipher.len() - Self::OVERHEAD_BASE];
        self.decrypt_with_nonce(sender, nonce, cipher, &mut plain)
            .map(|_| plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        CryptoBox::try_from((
            &id.to_encryption_key()?,
            self.encryption_key.private_key(),
        ))
        .map(|v| CryptoContext::new(id.clone(), v))
    }
}

#[cfg(test)]
#[path = "unitests/test_crypto_identity.rs"]
mod test_crypto_identity;
