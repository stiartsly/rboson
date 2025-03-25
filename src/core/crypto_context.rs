use std::convert::TryFrom;
use std::cell::RefCell;
use crate::{
    Id,
    cryptobox::{CryptoBox, Nonce, PrivateKey},
    error::Error
};

#[allow(dead_code)]
pub(crate) struct CryptoContext {
    id  : Id,
    box_: CryptoBox,

    next_nonce : RefCell<Nonce>,
}

#[allow(dead_code)]
impl CryptoContext {
    pub fn new(id: &Id, private_key: &PrivateKey) -> CryptoContext {
        let encryption_key = id.to_encryption_key();

        Self {
            id  : id.clone(),
            box_: CryptoBox::try_from((&encryption_key, private_key)).ok().unwrap(),

            next_nonce      : RefCell::new(Nonce::random())
        }
    }

    pub fn from_cryptobox(id: &Id, box_: CryptoBox) -> CryptoContext {
        Self {
            id  : id.clone(),
            box_,

            next_nonce      : RefCell::new(Nonce::random())
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
        if let Err(e) =  self.box_.encrypt(plain,  &mut cipher, &nonce) {
            return Err(e);
        }
        Ok(cipher)
    }

    pub fn decrypt_into(&self, cipher: &[u8]) -> Result<Vec<u8>, Error> {
        //let nonce = Nonce::try_from(&cipher[0..Nonce::BYTES]).unwrap();
        self.box_.decrypt_into(&cipher)
    }
}

impl Drop for CryptoContext {
    fn drop(&mut self) {
        self.box_.clear();
        self.next_nonce.borrow_mut().clear();
    }
}
