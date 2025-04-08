use crate::{
    Id,
    core::crypto_context::CryptoContext,
    Error,
    error::Result
};

pub trait Identity {
    fn id(&self) -> &Id;

    fn sign(&self, _data: &[u8], _signature: &mut [u8]) -> Result<usize> {
        Err(Error::NotImplemented("sign".to_string()))
    }

    fn sign_into(&self, _data: &[u8]) -> Result<Vec<u8>> {
        Err(Error::NotImplemented("sign_into".to_string()))
    }

    fn verify(&self, _data: &[u8], _signature: &[u8]) -> Result<()> {
        Err(Error::NotImplemented("verify".to_string()))
    }

    fn encrypt(&self, _recipient: &Id, _plain: &[u8], _cipher: &mut [u8]) -> Result<usize> {
        Err(Error::NotImplemented("encrypt".to_string()))
    }

    fn decrypt(&self, _sender: &Id, _cipher: &[u8], _plain: &mut [u8]) -> Result<usize> {
        Err(Error::NotImplemented("decrypt".to_string()))
    }

    fn encrypt_into(&self, _recipient: &Id, _data: &[u8]) -> Result<Vec<u8>> {
        Err(Error::NotImplemented("encrypt_into".to_string()))
    }

    fn decrypt_into(&self, _sender: &Id, _data: &[u8]) -> Result<Vec<u8>> {
        Err(Error::NotImplemented("decrypt_into".to_string()))
    }

    fn create_crypto_context(&self, _id: &Id) -> Result<CryptoContext> {
        Err(Error::NotImplemented("create_crypto_context".to_string()))
    }
}
