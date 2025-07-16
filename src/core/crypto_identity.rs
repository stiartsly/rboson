use super::{
    Id,
    cryptobox::{self, Nonce, CryptoBox},
    signature,
    Result,
    Identity,
    CryptoContext
};

#[derive(Clone, Debug)]
pub struct CryptoIdentity {
    id: Id,
    encrypt_keypair: cryptobox::KeyPair,
    signature_keypair: signature::KeyPair,
}

impl AsRef<CryptoIdentity> for CryptoIdentity {
    fn as_ref(&self) -> &CryptoIdentity {
        self
    }
}

impl CryptoIdentity {
    pub fn new() -> CryptoIdentity {
        Self::from_keypair(signature::KeyPair::random())
    }

    pub fn from_private_key(private_key: &[u8]) -> Result<CryptoIdentity> {
        Ok(Self::from_keypair(
            signature::KeyPair::try_from(private_key)?
        ))
    }

    pub fn from_keypair(keypair: signature::KeyPair) -> CryptoIdentity {
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

    pub fn id(&self) -> &Id {
        Identity::id(self)
    }

    pub fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        Identity::sign_into(self, data)
    }

    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        Identity::verify(self, data, signature)
    }

    pub fn encrypt_into(&self, recipient: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        Identity::encrypt_into(self, recipient, plain)
    }

    pub fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        Identity::decrypt_into(self, sender, cipher)
    }
}

impl PartialEq for CryptoIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Identity for CryptoIdentity {
    type IdentityObject = CryptoIdentity;

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

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        let mut plain = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        self.decrypt(sender, cipher, &mut plain)?;
        Ok(plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        CryptoBox::try_from((&id.to_encryption_key(), self.encrypt_keypair.private_key())).map(|v|
            CryptoContext::new(id.clone(), v)
        )
    }
}
