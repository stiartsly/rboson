use std::convert::TryFrom;
use crate::{
    Id,
    cryptobox::{CryptoBox, Nonce, PrivateKey},
    Error,
    error::Result
};

#[derive(Debug)]
pub struct CryptoContext {
    id  : Id,
    box_: CryptoBox,

    next_nonce: Nonce,
    last_peer_nonce: Option<Nonce>,
}

unsafe impl Send for CryptoContext {}
impl CryptoContext {
    pub(crate) fn new(id: &Id, private_key: &PrivateKey) -> CryptoContext {
        let encryption_key = id.to_encryption_key();

        Self {
            id: id.clone(),
            box_: CryptoBox::try_from((&encryption_key, private_key)).ok().unwrap(),
            next_nonce: Nonce::random(),
            last_peer_nonce: None
        }
    }

    pub(crate) fn from_cryptobox(id: &Id, box_: CryptoBox) -> CryptoContext {
        Self {
            id: id.clone(),
            box_,
            next_nonce: Nonce::random(),
            last_peer_nonce: None
        }
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
        let nonce = self.increment_nonce();
        self.box_.encrypt(plain, cipher, &nonce)
    }

    pub fn encrypt_into(&mut self, plain: &[u8]) -> Result<Vec<u8>> {
        let nonce = self.increment_nonce();
        self.box_.encrypt_into(plain, &nonce)
    }

    pub fn decrypt(&self, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        let nonce = &cipher[..Nonce::BYTES];
        if let Some(last_nonce) = self.last_peer_nonce.as_ref() {
            if last_nonce.as_bytes() != nonce {
                return Err(Error::Crypto("Using inconsistent nonce with risking of replay attacks".to_string()));
            }
        }
        self.box_.decrypt(cipher, plain)
    }

    pub fn decrypt_into(&self, cipher: &[u8]) -> Result<Vec<u8>> {
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
        self.next_nonce.clear();
    }
}
