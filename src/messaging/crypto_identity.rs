use crate::{
    Id,
    signature,
    cryptobox,
    error::Result
};

trait Identity {
    fn id(&self) -> &Id;

    fn sign(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()>;

    fn encrypt(&self, recipient: &Id, data: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>>;

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
        let keypair = signature::KeyPair::random();
        Self::from(&keypair)
    }

    pub(crate) fn from(keypair: signature::KeyPair) -> Self {
        Self {
            id: Id::from(keypair.public_key()),
            encryption_keypair: cryptobox::KeyPair::from(keypair),
            keypair: keypair
        }
    }
}

impl Identity for CryptoIdentity {
    pub(crate) fn id(&self) -> &Id {
        &self.id
    }

    pub(crate) fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        signature::sign_into(data, self.keypair.private_key())
    }

    pub(crate) fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        signature::verify(data, signature, self.keypair.public_key())
    }

    // one-shot encryption
    pub(crate) fn encrypt_into(&self, recipient: &Id, data: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }

    // one-short decryption
    pub(crate) fn decrypt_into(&self, _sender: &Id, _encrypted: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }
}
