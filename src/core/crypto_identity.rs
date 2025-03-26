use crate::{
    Id,
    cryptobox,
    signature,
    Identity,
    core::crypto_context::CryptoContext,
    error::Result
};

#[allow(dead_code)]
pub(crate) struct CryptoIdentity {
    id: Id,
    keypair: cryptobox::KeyPair,
    signature_keypair: signature::KeyPair,
}

#[allow(dead_code)]
impl CryptoIdentity {
    pub(crate) fn new() -> CryptoIdentity {
        Self::from_keypair(signature::KeyPair::random())
    }

    pub(crate) fn from_private_key(private_key: &signature::PrivateKey) -> CryptoIdentity {
        Self::from_keypair(signature::KeyPair::from(private_key))
    }

    pub(crate) fn from_keypair(keypair: signature::KeyPair) -> CryptoIdentity {
        Self {
            id: Id::from(keypair.to_public_key()),
            keypair: cryptobox::KeyPair::from(&keypair),
            signature_keypair: keypair,
        }
    }
}

impl Identity for CryptoIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        signature::sign_into(data, self.signature_keypair.private_key())
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        signature::verify(data, signature, self.signature_keypair.public_key())
    }

    fn encrypt_into(&self, recipient: &Id, data: &[u8]) -> Result<Vec<u8>> {
        let nonce = cryptobox::Nonce::random();
        let pk = recipient.to_encryption_key();
        let sk = self.keypair.private_key();
        cryptobox::encrypt_into(data, &nonce, &pk, sk)
    }

    fn decrypt_into(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>> {
        let pk = sender.to_encryption_key();
        let sk = self.keypair.private_key();
        cryptobox::decrypt_into(data, &pk, sk)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        let pk = id.to_encryption_key();
        cryptobox::CryptoBox::try_from((&pk, self.keypair.private_key())).map(|box_| {
            CryptoContext::from_cryptobox(id, box_)
        })
    }
}
