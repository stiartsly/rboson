use crate::{
    Id,
    cryptobox::{self, Nonce, CryptoBox},
    signature,
    error::Result,
    Identity,
    core::crypto_context::CryptoContext
};

#[derive(Clone, Debug)]
pub struct CryptoIdentity {
    id: Id,
    encrypt_keypair: cryptobox::KeyPair,
    signature_keypair: signature::KeyPair,
}

impl CryptoIdentity {
    #[allow(unused)]
    pub(crate) fn new() -> CryptoIdentity {
        Self::from_keypair(signature::KeyPair::random())
    }

    #[allow(unused)]
    pub(crate) fn from_private_key(private_key: &signature::PrivateKey) -> CryptoIdentity {
        Self::from_keypair(signature::KeyPair::from(private_key))
    }

    pub(crate) fn from_keypair(keypair: signature::KeyPair) -> CryptoIdentity {
        Self {
            id: Id::from(keypair.to_public_key()),
            encrypt_keypair: cryptobox::KeyPair::from(&keypair),
            signature_keypair: keypair,
        }
    }

    pub fn keypair(&self) -> &signature::KeyPair {
        &self.signature_keypair
    }

    pub fn encryption_keypair(&self) -> &cryptobox::KeyPair {
        &self.encrypt_keypair
    }
}

impl PartialEq for CryptoIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Identity for CryptoIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        signature::sign(data, signature, self.signature_keypair.private_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut signature = vec![0u8; signature::Signature::BYTES];
        self.sign(data, &mut signature)?;
        Ok(signature)
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        signature::verify(data, signature, self.signature_keypair.public_key())
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        cryptobox::encrypt(
            plain,
            cipher,
            &Nonce::random(),
            &recipient.to_encryption_key(),
            self.encrypt_keypair.private_key()
        )
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        cryptobox::decrypt(
            cipher,
            plain,
            &sender.to_encryption_key(),
            self.encrypt_keypair.private_key()
        )
    }

    fn encrypt_into(&self, recipient: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        self.encrypt(recipient, plain, &mut cipher)?;
        Ok(cipher)
    }

    fn decrypt_into(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>> {
        let mut plain = vec![0u8; data.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        self.decrypt(sender, data, &mut plain)?;
        Ok(plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        CryptoBox::try_from((&id.to_encryption_key(), self.encrypt_keypair.private_key())).map(|v|
            CryptoContext::from_cryptobox(id, v)
        )
    }
}
