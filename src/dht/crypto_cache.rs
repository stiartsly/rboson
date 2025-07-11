use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::SystemTime;

use crate::{
    elapsed_ms,
    Id
};

use crate::core::{
    cryptobox::KeyPair,
    crypto_context::CryptoContext,
};

pub(crate) const EXPIRED_CHECK_INTERVAL: u64 = 60 * 1000;
pub(crate) struct CryptoCache {
    keypair: KeyPair,
    cache: HashMap<Id, Arc<Mutex<Entry>>>,
}

impl CryptoCache {
    pub(crate) fn new(keypair: KeyPair) -> CryptoCache {
        Self {
            keypair,
            cache: HashMap::new(),
        }
    }

    pub(crate) fn get(&mut self, key: &Id) -> Arc<Mutex<Entry>>{
        let entry = self.cache.get(key);
        if let Some(entry) = entry {
            return entry.clone();
        } else {
            let entry = Arc::new(Mutex::new(Entry::new(self.load(key))));
            self.cache.insert(key.clone(), entry.clone());
            return entry;
        }
    }

    pub(crate) fn expire(&mut self) {
        let mut to_remove = vec![];
        self.cache.iter_mut().for_each(|(id, entry)| {
            if entry.lock().unwrap().expired() {
                to_remove.push(id.clone());
            }
        });

        to_remove.iter().for_each(|id| {
            self.cache.remove(id);
        });
    }

    fn load(&self, key: &Id) -> CryptoContext {
        CryptoContext::from_private_key(
            key.clone(),
            self.keypair.private_key()
        )
    }
}

pub(crate) struct Entry(CryptoContext, SystemTime);
impl Entry {
    fn new(value: CryptoContext) -> Self {
        Entry(value, SystemTime::now())
    }

    fn expired(&self) -> bool {
        elapsed_ms!(&self.1) >= EXPIRED_CHECK_INTERVAL as u128
    }

    pub(crate) fn ctx_mut(&mut self) -> &mut CryptoContext {
        &mut self.0
    }
}
