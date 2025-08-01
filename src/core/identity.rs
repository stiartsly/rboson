use crate::{
    Id,
    Error,
    CryptoContext,
    core::Result,
};

pub trait Identity {
    type IdentityObject;

    fn id(&self) -> &Id;

    fn sign(&self, _data: &[u8], _sig: &mut [u8]) -> Result<usize> {
        Err(Error::NotImplemented("sign".into()))
    }

    fn sign_into(&self, _data: &[u8]) -> Result<Vec<u8>> {
        Err(Error::NotImplemented("sign_into".into()))
    }

    fn verify(&self, _data: &[u8], _sig: &[u8]) -> Result<()> {
        Err(Error::NotImplemented("verify".into()))
    }

    fn encrypt(&self, _rec: &Id, _plain: &[u8], _cipher: &mut [u8]) -> Result<usize> {
        Err(Error::NotImplemented("encrypt".into()))
    }

    fn decrypt(&self, _sender: &Id, _cipher: &[u8], _plain: &mut [u8]) -> Result<usize> {
        Err(Error::NotImplemented("decrypt".into()))
    }

    fn encrypt_into(&self, _rec: &Id, _plain: &[u8]) -> Result<Vec<u8>> {
        Err(Error::NotImplemented("encrypt_into".into()))
    }

    fn decrypt_into(&self, _sender: &Id, _cipher: &[u8]) -> Result<Vec<u8>> {
        Err(Error::NotImplemented("decrypt_into".into()))
    }

    fn create_crypto_context(&self, _id: &Id) -> Result<CryptoContext> {
        Err(Error::NotImplemented("create_crypto_context".into()))
    }
}
