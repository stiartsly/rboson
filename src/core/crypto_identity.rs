use crate::{
    Id,
    cryptobox::{self, Nonce},
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

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        signature::sign(data, signature, self.signature_keypair.private_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut signature = vec![0u8; signature::Signature::BYTES];
        self.sign(data, &mut signature).map(|_| signature)
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
            self.keypair.private_key()
        )
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        cryptobox::decrypt(
            cipher,
            plain,
            &sender.to_encryption_key(),
            self.keypair.private_key()
        )
    }

    fn encrypt_into(&self, recipient: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + cryptobox::CryptoBox::MAC_BYTES + Nonce::BYTES];
        self.encrypt(recipient, plain, &mut cipher).map(|_| cipher)
    }

    fn decrypt_into(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>> {
        let mut plain = vec![0u8; data.len() - cryptobox::CryptoBox::MAC_BYTES - Nonce::BYTES];
        self.decrypt(sender, data, &mut plain).map(|_| plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        cryptobox::CryptoBox::try_from((
            &id.to_encryption_key(),
            self.keypair.private_key()
        )).map(|box_| {
            CryptoContext::from_cryptobox(id, box_)
        })
    }
}
