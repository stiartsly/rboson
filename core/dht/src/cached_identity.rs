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

use moka::sync::Cache;
use std::sync::{Arc, Mutex};

use boson_api::{
    {CryptoContext, CryptoIdentity, Result},
    errors::CryptoError,
    Id, Identity,
};

const CONTEXT_CACHE_CAPACITY: u64 = 10;

pub(crate) struct CachedIdentity {
    id: Id,
    identity: Arc<CryptoIdentity>,
    cache: Cache<Id, Arc<Mutex<CryptoContext>>>,
}

impl CachedIdentity {
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        Self {
            id: identity.id().clone(),
            identity: Arc::new(identity),
            cache: Cache::new(CONTEXT_CACHE_CAPACITY),
        }
    }

    pub(crate) fn clear_cache(&self) {
        self.cache.invalidate_all();
    }

    /// Fails when `key` is not a usable public key, as an id from the network may not be.
    pub(crate) fn context(&self, key: &Id) -> Result<Arc<Mutex<CryptoContext>>> {
        self.cache
            .try_get_with(key.clone(), || {
                CryptoContext::from_private_key(key.clone(), self.identity.encryption_key().private_key())
                    .map(|context| Arc::new(Mutex::new(context)))
                    .map_err(|e| e.to_string())
            })
            .map_err(|e| CryptoError::new(e.to_string()).into())
    }

    pub(crate) fn identity(&self) -> Arc<CryptoIdentity> {
        self.identity.clone()
    }
}

impl Identity for CachedIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        self.identity.sign(data, signature)
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.identity.sign_into(data)
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        self.identity.verify(data, signature)
    }

    fn encrypt(&self, receiver: &Id, data: &[u8], cipher: &mut [u8]) -> Result<usize> {
        self.context(receiver)?.lock().unwrap().encrypt(data, cipher)
    }

    fn encrypt_into(&self, receiver: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        self.context(receiver)?.lock().unwrap().encrypt_into(plain)
    }

    fn decrypt(&self, sender: &Id, data: &[u8], plain: &mut [u8]) -> Result<usize> {
        self.context(sender)?.lock().unwrap().decrypt(data, plain)
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        self.context(sender)?.lock().unwrap().decrypt_into(cipher)
    }

    fn encrypt_with_nonce(
        &self,
        receiver: &Id,
        nonce: &[u8],
        data: &[u8],
        cipher: &mut [u8],
    ) -> Result<usize> {
        self.identity.encrypt_with_nonce(receiver, nonce, data, cipher)
    }

    fn decrypt_with_nonce(
        &self,
        sender: &Id,
        nonce: &[u8],
        data: &[u8],
        plain: &mut [u8],
    ) -> Result<usize> {
        self.identity.decrypt_with_nonce(sender, nonce, data, plain)
    }

    fn encrypt_with_nonce_into(
        &self,
        receiver: &Id,
        nonce: &[u8],
        data: &[u8],
    ) -> Result<Vec<u8>> {
        self.identity.encrypt_with_nonce_into(receiver, nonce, data)
    }

    fn decrypt_with_nonce_into(
        &self,
        sender: &Id,
        nonce: &[u8],
        data: &[u8],
    ) -> Result<Vec<u8>> {
        self.identity.decrypt_with_nonce_into(sender, nonce, data)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        self.identity.create_crypto_context(id)
    }
}

impl Drop for CachedIdentity {
    fn drop(&mut self) {
        self.clear_cache();
    }
}
