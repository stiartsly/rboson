use std::convert::TryFrom;
use std::cell::RefCell;
use crate::{
    Id,
    cryptobox::{CryptoBox, Nonce, PrivateKey},
    error::Error
};

#[allow(dead_code)]
pub struct CryptoContext {
    id  : Id,
    box_: CryptoBox,

    next_nonce: RefCell<Nonce>,
    last_peer_nonce: Option<Nonce>,
}

#[allow(dead_code)]
impl CryptoContext {
    pub(crate) fn new(id: &Id, private_key: &PrivateKey) -> CryptoContext {
        let encryption_key = id.to_encryption_key();

        Self {
            id: id.clone(),
            box_: CryptoBox::try_from((&encryption_key, private_key)).ok().unwrap(),
            next_nonce: RefCell::new(Nonce::random()),
            last_peer_nonce: None
        }
    }

    pub(crate) fn from_cryptobox(id: &Id, box_: CryptoBox) -> CryptoContext {
        Self {
            id: id.clone(),
            box_,
            next_nonce: RefCell::new(Nonce::random()),
            last_peer_nonce: None
        }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    fn increment_nonce(&self) -> Nonce {
        let nonce = self.next_nonce.borrow().clone();
        self.next_nonce.borrow_mut().increment();
        nonce
    }

    pub fn encrypt_into(&self, plain: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = self.increment_nonce();
        let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        self.box_.encrypt(plain,  &mut cipher, &nonce).map(|_| cipher)
    }

    pub fn decrypt_into(&self, cipher: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = &cipher[..Nonce::BYTES];
        if let Some(last_nonce) = self.last_peer_nonce.as_ref() {
            if last_nonce.as_bytes() != nonce {
                return Err(Error::Crypto("Using inconsistent nonce with risking of replay attacks".to_string()));
            }
        }
        self.box_.decrypt_into(&cipher)
    }
}

impl Drop for CryptoContext {
    fn drop(&mut self) {
        self.box_.clear();
        self.next_nonce.borrow_mut().clear();
    }
}
