use super::{
    cryptobox::{self, CryptoBox, Nonce},
    signature::{self, Signature},
    CryptoContext,
    Id,
    identity::Identity,
    errors::{ArgumentError, Result},
};
use std::hash;

#[derive(Clone)]
pub struct CryptoIdentity {
    id: Id,
    signature_key: signature::KeyPair,
    encryption_key: cryptobox::KeyPair,
}

impl CryptoIdentity {
    const OVERHEAD_WITH_NONCE:  usize = CryptoBox::MAC_BYTES + Nonce::BYTES;
    const OVERHEAD_NO_NONCE:    usize = CryptoBox::MAC_BYTES;

    fn from_keypair(keypair: signature::KeyPair) -> CryptoIdentity {
        Self {
            id              : Id::from(keypair.public_key()),
            encryption_key  : cryptobox::KeyPair::from(&keypair),
            signature_key   : keypair,
        }
    }

    pub fn new() -> Self {
        Self::from_keypair(signature::KeyPair::random())
    }

    pub fn random() -> Self {
        Self::from_keypair(signature::KeyPair::random())
    }

    pub fn signature_key(&self) -> &signature::KeyPair {
        &self.signature_key
    }

    pub fn encryption_key(&self) -> &cryptobox::KeyPair {
        &self.encryption_key
    }
}

impl CryptoIdentity {
    fn crypto_box_for(&self, id: &Id) -> Result<CryptoBox> {
        CryptoBox::try_from((&id.to_encryption_key(), self.encryption_key.private_key()))
    }
}

impl Default for CryptoIdentity {
    fn default() -> Self {
        Self::random()
    }
}

impl Eq for CryptoIdentity {}

impl PartialEq for CryptoIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl hash::Hash for CryptoIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl TryFrom<&[u8]> for CryptoIdentity {
    type Error = super::Error;

    fn try_from(private_key: &[u8]) -> Result<Self> {
        signature::KeyPair::try_from(private_key).map(Self::from_keypair)
    }
}

impl From<signature::KeyPair> for CryptoIdentity {
    fn from(keypair: signature::KeyPair) -> Self {
        Self::from_keypair(keypair)
    }
}

impl From<signature::PrivateKey> for CryptoIdentity {
    fn from(private_key: signature::PrivateKey) -> Self {
        Self::from_keypair(signature::KeyPair::from(private_key))
    }
}

impl AsRef<CryptoIdentity> for CryptoIdentity {
    fn as_ref(&self) -> &CryptoIdentity {
        self
    }
}

impl Drop for CryptoIdentity {
    fn drop(&mut self) {
        self.signature_key.clear();
        self.encryption_key.clear()
    }
}

impl Identity for CryptoIdentity {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], sig: &mut [u8]) -> Result<usize> {
        signature::sign(data, sig, self.signature_key.private_key())
    }

    fn verify(&self, data: &[u8], sig: &[u8]) -> Result<bool> {
        signature::verify(data, sig, self.signature_key.public_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut v = vec![0u8; Signature::BYTES];
        self.sign(data, &mut v).map(|_| v)
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        if cipher.len() < plain.len() + Self::OVERHEAD_WITH_NONCE {
            return Err(ArgumentError::new("The output buffer is insufficient."));
        }
        let nonce = Nonce::random();
        cipher[..Nonce::BYTES].copy_from_slice(nonce.as_bytes());
        let written = self.encrypt_with_nonce(
            recipient,
            nonce.as_bytes(),
            plain,
            &mut cipher[Nonce::BYTES..],
        )?;
        Ok(Nonce::BYTES + written)
    }

    fn encrypt_into(&self, rec: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + Self::OVERHEAD_WITH_NONCE];
        self.encrypt(rec, plain, &mut cipher).map(|_| cipher)
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        if cipher.len() < Self::OVERHEAD_WITH_NONCE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} is smaller than the required overhead {}",
                cipher.len(),
                Self::OVERHEAD_WITH_NONCE
            )));
        }
        let nonce = &cipher[..Nonce::BYTES];
        self.decrypt_with_nonce(sender, nonce, &cipher[Nonce::BYTES..], plain)
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        if cipher.len() < Self::OVERHEAD_WITH_NONCE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} is smaller than the required overhead {}",
                cipher.len(),
                Self::OVERHEAD_WITH_NONCE
            )));
        }
        let mut plain = vec![0u8; cipher.len() - Self::OVERHEAD_WITH_NONCE];
        self.decrypt(sender, cipher, &mut plain).map(|_| plain)
    }

    fn encrypt_with_nonce(
        &self,
        recipient: &Id,
        nonce: &[u8],
        plain: &[u8],
        cipher: &mut [u8],
    ) -> Result<usize> {
        if cipher.len() < plain.len() + Self::OVERHEAD_NO_NONCE {
            return Err(ArgumentError::new("The output buffer is insufficient."));
        }

        let nonce = Nonce::try_from(nonce)?;
        self.crypto_box_for(recipient)?.encrypt(plain, cipher, &nonce)
    }

    fn decrypt_with_nonce(
        &self,
        sender: &Id,
        nonce: &[u8],
        cipher: &[u8],
        plain: &mut [u8],
    ) -> Result<usize> {
        if cipher.len() < Self::OVERHEAD_NO_NONCE {
            return Err(ArgumentError::new(format!(
                "Ciphertext length {} is smaller than the required overhead {}",
                cipher.len(),
                Self::OVERHEAD_NO_NONCE
            )));
        }
        let nonce = Nonce::try_from(nonce)?;
        self.crypto_box_for(sender)?.decrypt(cipher, plain, &nonce)
    }

    fn encrypt_with_nonce_into(
        &self,
        rec: &Id,
        nonce: &[u8],
        plain: &[u8],
    ) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + Self::OVERHEAD_NO_NONCE];
        self.encrypt_with_nonce(rec, nonce, plain, &mut cipher)
            .map(|_| cipher)
    }

    fn decrypt_with_nonce_into(
        &self,
        sender: &Id,
        nonce: &[u8],
        cipher: &[u8],
    ) -> Result<Vec<u8>> {
        if cipher.len() < Self::OVERHEAD_NO_NONCE {
            return Err(ArgumentError::new("Invalid cipher length"));
        }
        let mut plain = vec![0u8; cipher.len() - Self::OVERHEAD_NO_NONCE];
        self.decrypt_with_nonce(sender, nonce, cipher, &mut plain)
            .map(|_| plain)
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        CryptoBox::try_from((
            &id.to_encryption_key(),
            self.encryption_key.private_key(),
        ))
        .map(|v| CryptoContext::new(id.clone(), v))
    }
}

#[cfg(test)]
#[path = "unitests/test_crypto_identity.rs"]
mod test_crypto_identity;
