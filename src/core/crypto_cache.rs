use std::rc::Rc;
use std::collections::HashMap;
use std::time::SystemTime;

use crate::{
    as_millis,
    Id
};

use crate::core::{
    cryptobox::KeyPair,
    crypto_context::CryptoContext,
};

pub(crate) const EXPIRED_CHECK_INTERVAL: u64 = 60 * 1000;
pub(crate) struct CryptoCache {
    keypair: KeyPair,
    cache: HashMap<Id, Rc<Entry>>,
}

impl CryptoCache {
    pub(crate) fn new(keypair: KeyPair) -> CryptoCache {
        Self {
            keypair,
            cache: HashMap::new(),
        }
    }

    pub(crate) fn get(&mut self, key: &Id) -> Rc<Entry> {
        let entry = self.cache.get(key);
        if entry.is_none() {
            let _new = Rc::new(Entry::new(self.load(key)));
            self.cache.insert(key.clone(), _new.clone());
            return _new
        }
        entry.unwrap().clone()
    }

    pub(crate) fn expire(&mut self) {
        let mut to_remove = vec![];
        self.cache.iter_mut().for_each(|(id, entry)| {
            if entry.expired() {
                to_remove.push(id.clone());
            }
        });

        to_remove.iter().for_each(|id| {
            self.cache.remove(id);
        });
    }

    fn load(&self, key: &Id) -> Box<CryptoContext> {
        Box::new(CryptoContext::new(
            key,
            self.keypair.private_key()
        ))
    }
}

pub(crate) struct Entry(Box<CryptoContext>, SystemTime);
impl Entry {
    fn new(value: Box<CryptoContext>) -> Self {
        Entry(value, SystemTime::now())
    }

    fn expired(&self) -> bool {
        as_millis!(&self.1) >= EXPIRED_CHECK_INTERVAL as u128
    }

    pub(crate) fn ctx(&self) -> &Box<CryptoContext> {
        &self.0
    }
}

/*
pub(crate) struct CryptoContext {
    box_: CryptoBox,
    nonce: Nonce,
}

impl CryptoContext {
    fn new(pk: &PublicKey, keypair: &KeyPair) -> CryptoContext {
        let receiver = Id::try_from(pk.as_bytes()).unwrap();
        let sender   = Id::try_from(keypair.public_key().as_bytes()).unwrap();
        let distance = Id::distance(&sender, &receiver);

        Self {
            box_: CryptoBox::try_from((pk, keypair.private_key())).unwrap(),
            nonce: Nonce::try_from(&distance.as_bytes()[0..Nonce::BYTES]).unwrap(),
        }
    }

    pub(crate) fn encrypt_into(&self, plain: &[u8]) -> Result<Vec<u8>, Error> {
        self.box_.encrypt_into(plain, &self.nonce)
    }

    pub(crate) fn decrypt_into(&self, cipher: &[u8]) -> Result<Vec<u8>, Error> {
        self.box_.decrypt_into(cipher, &self.nonce)
    }
}

impl Drop for CryptoContext {
    fn drop(&mut self) {
        self.box_.clear();
        self.nonce.clear();
    }
}
*/
