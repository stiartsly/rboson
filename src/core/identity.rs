use super::{
    Id,
    CryptoContext,
    cryptobox::Nonce,
    cryptobox::{self, CryptoBox},
    signature::{self, Signature},
    errors::{ArgumentError, Result},
};

/// An entity capable of signing, verifying, encrypting, and decrypting.
pub trait Identity {
    /// Returns the identifier of this identity.
    fn id(&self) -> &Id;

    /// Signs `data` with this identity's private signing key.
    ///
    /// Writes the 64-byte Ed25519 signature to `sig` and returns the number of
    /// bytes written.
    fn sign(&self, _data: &[u8], _sig: &mut [u8]) -> Result<usize>;

    /// Verifies that `sig` is a valid signature over `data` for this identity.
    ///
    /// Returns `Ok(true)` for a valid signature and `Ok(false)` for an invalid
    /// signature.
    fn verify(&self, _data: &[u8], _sig: &[u8]) -> Result<bool>;

    /// Signs `data` and returns the 64-byte Ed25519 signature in a new vector.
    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>>;

    /// Encrypts `plain` for `recipient` using this identity's private encryption key.
    ///
    /// Prepends a random nonce to `cipher` and returns the number of bytes written.
    fn encrypt(&self, _rec: &Id, _plain: &[u8], _cipher: &mut [u8]) -> Result<usize>;

    /// Decrypts `cipher` received from `sender` into `plain`.
    ///
    /// The ciphertext must begin with the nonce written by [`Identity::encrypt`].
    /// Returns the number of plaintext bytes written.
    fn decrypt(&self, _sender: &Id, _cipher: &[u8], _plain: &mut [u8]) -> Result<usize>;

    /// Encrypts `plain` for `rec` and returns nonce-prefixed ciphertext in a new vector.
    fn encrypt_into(&self, rec: &Id, plain: &[u8]) -> Result<Vec<u8>>;

    /// Decrypts nonce-prefixed `cipher` from `sender` and returns the plaintext in a new vector.
    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>>;

    /// Creates a reusable encryption context for communication with the identity `id`.
    ///
    /// The resulting context combines this identity's private encryption key with
    /// the peer's public encryption key.
    fn create_crypto_context(&self, _id: &Id) -> Result<CryptoContext>;
}

#[derive(Clone)]
pub struct CryptoIdentity {
    id      : Id,
    keypair : signature::KeyPair,
    encryption_keypair: cryptobox::KeyPair,
}

impl CryptoIdentity {
    const CIPHER_OVERHEAD: usize = CryptoBox::MAC_BYTES + Nonce::BYTES;

     pub fn new() -> Self {
        Self::from(signature::KeyPair::random())
    }

    pub fn from(keypair: signature::KeyPair) -> CryptoIdentity {
        let encryption_kp = cryptobox::KeyPair::from(&keypair);
        Self {
            id: Id::from(keypair.public_key()),
            keypair: keypair,
            encryption_keypair: encryption_kp
        }
    }

    pub fn signature_keypair(&self) -> &signature::KeyPair {
        &self.keypair
    }

    pub fn encryption_keypair(&self) -> &cryptobox::KeyPair {
        &self.encryption_keypair
    }
}

impl Identity for CryptoIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        signature::sign(data, signature, self.keypair.private_key())
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        signature::verify(data, signature, self.keypair.public_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; Signature::BYTES];
        self.sign(data, &mut v).map(|_| v)
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        cryptobox::encrypt(
            plain,
            cipher,
            &Nonce::random(),
            &recipient.to_encryption_key(),
            self.encryption_keypair.private_key()
        )
    }

    fn encrypt_into(&self, rec: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        self.encrypt(rec, plain, &mut v).map(|_| v)
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        if cipher.len() < Self::CIPHER_OVERHEAD {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} is smaller than the required overhead {}",
                cipher.len(), Self::CIPHER_OVERHEAD
            )));
        }

        cryptobox::decrypt(
            cipher,
            plain,
            &sender.to_encryption_key(),
            self.encryption_keypair.private_key()
        )
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        self.decrypt(sender, cipher, &mut v).map(|_| v)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        CryptoBox::try_from(
            (&id.to_encryption_key(), self.encryption_keypair.private_key())
        ).map(|v|
            CryptoContext::new(id.clone(), v)
        )
    }
}

impl TryFrom<&[u8]> for CryptoIdentity {
    type Error = super::Error;

    fn try_from(private_key: &[u8]) -> Result<Self> {
        signature::KeyPair::try_from(private_key).map(Self::from)
    }
}

impl From<signature::PrivateKey> for CryptoIdentity {
    fn from(private_key: signature::PrivateKey) -> Self {
        Self::from(signature::KeyPair::from(private_key))
    }
}

impl AsRef<CryptoIdentity> for CryptoIdentity {
    fn as_ref(&self) -> &CryptoIdentity {
        self
    }
}

impl Drop for CryptoIdentity {
    fn drop(&mut self) {
        self.keypair.clear();
        self.encryption_keypair.clear()
    }
}
