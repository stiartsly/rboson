use std::sync::{Arc, Mutex};
use moka::sync::Cache;

use crate::{
    Id,
    Identity,
    core::{
        Result,
        crypto_identity::CryptoIdentity,
        crypto_context::CryptoContext,
    }
};

const CONTEXT_CACHE_CAPACITY: u64 = 10;

pub(crate) struct CachedIdentity {
    id: Id,
    identity: Arc<Mutex<CryptoIdentity>>,
    cache:  Mutex<Cache<Id, Arc<Mutex<CryptoContext>>>>,
}

impl CachedIdentity {
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        Self {
            id: identity.id().clone(),
            identity: Arc::new(Mutex::new(identity)),
            cache: Mutex::new(Cache::<Id, Arc<Mutex<CryptoContext>>>::new(CONTEXT_CACHE_CAPACITY)),
        }
    }

    pub(crate) fn clear_cache(&self) {
        self.cache.lock().unwrap().invalidate_all();
    }

    pub(crate) fn context(&self, key: &Id) -> Arc<Mutex<CryptoContext>> {
        self.cache.lock().unwrap().get_with(key.clone(), || {
            Arc::new(Mutex::new(CryptoContext::from_private_key(
                key.clone(),
                self.identity.lock().unwrap().encryption_keypair().private_key(),
            )))
        })
    }

    pub(crate) fn identity(&self) -> Arc<Mutex<CryptoIdentity>> {
        self.identity.clone()
    }
}

impl Drop for CachedIdentity {
    fn drop(&mut self) {
        self.clear_cache();
    }
}

impl Identity for CachedIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        self.identity.lock().unwrap().sign(data, signature)
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        self.identity.lock().unwrap().verify(data, signature)
    }

    fn encrypt(&self, receiver: &Id, data: &[u8], cipher: &mut [u8]) -> Result<usize> {
        self.context(receiver).lock().unwrap().encrypt(data, cipher)
    }

    fn decrypt(&self, sender: &Id, data: &[u8], plain: &mut [u8]) -> Result<usize> {
        self.context(sender).lock().unwrap().decrypt(data, plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        Ok(self.context(id).lock().unwrap().clone())
    }
}
