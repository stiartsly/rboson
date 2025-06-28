use crate::{
    Id,
    Error,
    core::Result,
    cryptobox::{CryptoBox, Nonce, PrivateKey}
};

#[derive(Debug)]
pub struct CryptoContext {
    id          : Id,
    crypto_box  : CryptoBox,
    next_nonce  : Nonce,
    last_peer_nonce: Option<Nonce>,
}

unsafe impl Send for CryptoContext {}
impl CryptoContext {
    pub(crate) fn new(id: Id, crypto_box: CryptoBox) -> CryptoContext {
        Self {
            id,
            crypto_box,
            next_nonce  : Nonce::random(),
            last_peer_nonce: None
        }
    }

    pub(crate) fn from_private_key(id: Id, pk: &PrivateKey) -> CryptoContext {
        let crypto_box = CryptoBox::try_from((&id.to_encryption_key(), pk)).unwrap();
        Self {
            id,
            crypto_box,
            next_nonce  : Nonce::random(),
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
        self.crypto_box.encrypt(plain, cipher, &nonce)
    }

    pub fn encrypt_into(&mut self, plain: &[u8]) -> Result<Vec<u8>> {
        let nonce = self.increment_nonce();
        self.crypto_box.encrypt_into(plain, &nonce)
    }

    pub fn decrypt(&self, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        let nonce = &cipher[..Nonce::BYTES];
        if let Some(last_nonce) = self.last_peer_nonce.as_ref() {
            if last_nonce.as_bytes() != nonce {
                return Err(Error::Crypto("Using inconsistent nonce with risking of replay attacks".into()));
            }
        }
        self.crypto_box.decrypt(cipher, plain)
    }

    pub fn decrypt_into(&self, cipher: &[u8]) -> Result<Vec<u8>> {
        let nonce = &cipher[..Nonce::BYTES];
        if let Some(last_nonce) = self.last_peer_nonce.as_ref() {
            if last_nonce.as_bytes() != nonce {
                return Err(Error::Crypto("Using inconsistent nonce with risking of replay attacks".into()));
            }
        }
        self.crypto_box.decrypt_into(&cipher)
    }
}

impl Drop for CryptoContext {
    fn drop(&mut self) {
        self.crypto_box.clear();
        self.next_nonce.clear();
    }
}
