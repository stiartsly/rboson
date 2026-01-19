use crate::{
    Id,
    CryptoContext,
    CryptoBox,
    cryptobox::Nonce,
    Signature,
    core::Result,
};

pub trait Identity {
    fn id(&self) -> &Id;
    fn sign(&self, _data: &[u8], _sig: &mut [u8]) -> Result<usize>;
    fn verify(&self, _data: &[u8], _sig: &[u8]) -> Result<()>;

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; Signature::BYTES];
        self.sign(data, &mut v).map(|_| v)
    }

    fn encrypt(&self, _rec: &Id, _plain: &[u8], _cipher: &mut [u8]) -> Result<usize>;
    fn decrypt(&self, _sender: &Id, _cipher: &[u8], _plain: &mut [u8]) -> Result<usize>;

    fn encrypt_into(&self, rec: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        self.encrypt(rec, plain, &mut v).map(|_| v)
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        self.decrypt(sender, cipher, &mut v).map(|_| v)
    }

    fn create_crypto_context(&self, _id: &Id) -> Result<CryptoContext>;
}
