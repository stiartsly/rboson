use crate::{
    Id,
    core::crypto_context::CryptoContext,
    error::Result
};

#[allow(dead_code)]
pub trait Identity {
    fn id(&self) -> &Id;

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()>;

    fn encrypt_into(&self, recipient: &Id, data: &[u8]) -> Result<Vec<u8>>;
    fn decrypt_into(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>>;

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext>;
}
