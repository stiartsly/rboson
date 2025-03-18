use crate::{
    Id,
    signature,
    cryptobox,
    error::Result
};

pub trait Identity {
    fn id(&self) -> &Id;

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()>;

    fn encrypt_into(&self, recipient: &Id, data: &[u8]) -> Result<Vec<u8>>;
    fn decrypt_into(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>>;

    //public CryptoContext createCryptoContext(Id id);
}

#[allow(dead_code)]
pub(crate) struct CryptoIdentity {
    id: Id,
    keypair: signature::KeyPair,
    encryption_keypair: cryptobox::KeyPair
}

#[allow(dead_code)]
impl CryptoIdentity {
    pub(crate) fn new() -> Self {
        Self::from(signature::KeyPair::random())
    }

    pub(crate) fn from(keypair: signature::KeyPair) -> Self {
        Self {
            id: Id::from(keypair.public_key().clone()),
            encryption_keypair: cryptobox::KeyPair::from(&keypair),
            keypair: keypair
        }
    }
}

impl Identity for CryptoIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        signature::sign_into(data, self.keypair.private_key())
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        signature::verify(data, signature, self.keypair.public_key())
    }

    // one-shot encryption
    fn encrypt_into(&self, _recipient: &Id, _data: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }

    // one-short decryption
    fn decrypt_into(&self, _sender: &Id, _encrypted: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }
}
