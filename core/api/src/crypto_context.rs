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

use std::sync::Mutex;

use crate::{
    cryptobox::{self as crypto_box, CryptoBox, Nonce, PrivateKey},
    errors::CryptoError,
    Id, Result,
};

#[derive(Debug)]
pub struct CryptoContext {
    id: Id,
    crypto_box: CryptoBox,
    next_nonce: Nonce,
    last_peer_nonce: Mutex<Option<Nonce>>,
}

impl Clone for CryptoContext {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            crypto_box: self.crypto_box.clone(),
            next_nonce: self.next_nonce.clone(),
            last_peer_nonce: Mutex::new(self.last_peer_nonce.lock().unwrap().clone()),
        }
    }
}

impl CryptoContext {
    const OVERHEAD: usize = Nonce::BYTES + CryptoBox::MAC_BYTES;

    pub fn new(id: Id, crypto_box: CryptoBox) -> Self {
        Self {
            id,
            crypto_box,
            next_nonce: Nonce::random(),
            last_peer_nonce: Mutex::new(None),
        }
    }

    pub fn from_keys(
        id: Id,
        pk: &crypto_box::PublicKey,
        sk: &PrivateKey,
    ) -> Result<Self> {
        Ok(Self::new(id, CryptoBox::try_from((pk, sk))?))
    }

    /// Fails when `id` is not a usable public key, as an id from the network may not be.
    pub fn from_private_key(id: Id, sk: &PrivateKey) -> Result<Self> {
        let pk = id.to_encryption_key()?;
        Self::from_keys(id, &pk, sk)
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    fn increment_nonce(&mut self) -> Nonce {
        let nonce = self.next_nonce.clone();
        self.next_nonce.increment();
        nonce
    }

    pub fn encrypt(&mut self, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        if cipher.len() < plain.len() + Self::OVERHEAD {
            return Err(CryptoError::new("The output buffer is insufficient"));
        }

        let nonce = self.increment_nonce();
        cipher[..Nonce::BYTES].copy_from_slice(nonce.as_bytes());

        let cipher  = &mut cipher[Nonce::BYTES..];
        let written = self.crypto_box.encrypt(plain, cipher, &nonce)?;
        Ok(Nonce::BYTES + written)
    }

    pub fn encrypt_into(&mut self, plain: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + Self::OVERHEAD];
        self.encrypt(plain, &mut cipher).map(|_| cipher)
    }

    pub fn decrypt(&self, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        if cipher.len() <= Self::OVERHEAD {
            return Err(CryptoError::new(format!(
                "Ciphertext length {} must be longer than the overhead {}",
                cipher.len(),
                Self::OVERHEAD
            )));
        }
        if plain.len() < cipher.len() - Self::OVERHEAD {
            return Err(CryptoError::new("The output buffer for plain text is insufficient"));
        }

        let nonce = Nonce::try_from(&cipher[..Nonce::BYTES])?;
        let mut last_nonce = self.last_peer_nonce.lock().unwrap();
        if last_nonce.as_ref().is_some_and(|last| last == &nonce) {
            return Err(CryptoError::new("Duplicated nonce"));
        }
        *last_nonce = Some(nonce.clone());

        let cipher = &cipher[Nonce::BYTES..];
        self.crypto_box.decrypt(cipher, plain, &nonce)
    }

    /// Clears the last peer nonce, allowing it to be received again.
    pub fn reset_nonce(&self) {
        *self.last_peer_nonce.lock().unwrap() = None;
    }

    /// Wipes the context's shared key; idempotent.
    pub fn close(&mut self) {
        self.crypto_box.clear();
        self.next_nonce.clear();
    }

    pub fn decrypt_into(&self, cipher: &[u8]) -> Result<Vec<u8>> {
        if cipher.len() <= Self::OVERHEAD {
            return Err(CryptoError::new(format!(
                "Ciphertext length {} must be longer than the overhead {}",
                cipher.len(),
                Self::OVERHEAD
            )));
        }
        let mut plain = vec![0u8; cipher.len() - Self::OVERHEAD];
        self.decrypt(cipher, &mut plain).map(|_| plain)
    }
}

impl Drop for CryptoContext {
    fn drop(&mut self) {
        self.crypto_box.clear();
        self.next_nonce.clear();
    }
}

#[cfg(test)]
#[path = "unitests/test_crypto_context.rs"]
mod test_crypto_context;
