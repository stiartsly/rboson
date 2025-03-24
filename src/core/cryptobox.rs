
use std::fmt;
use static_assertions::const_assert;
use libsodium_sys::{
    crypto_box_BEFORENMBYTES,
    crypto_box_MACBYTES,
    crypto_box_NONCEBYTES,
    crypto_box_PUBLICKEYBYTES,
    crypto_box_SECRETKEYBYTES,
    crypto_box_SEEDBYTES,
    crypto_box_beforenm,
    crypto_box_easy,
    crypto_box_easy_afternm,
    crypto_box_keypair,
    crypto_box_open_easy,
    crypto_box_open_easy_afternm,
    crypto_box_seed_keypair,
    crypto_scalarmult_base,
    crypto_sign_ed25519_pk_to_curve25519,
    crypto_sign_ed25519_sk_to_curve25519,
    sodium_increment,
};

use crate::{
    as_uchar_ptr,
    as_uchar_ptr_mut,
    randomize_bytes,
};

use crate::core::{
    signature,
    error::{Error, Result}
};

const_assert!(PrivateKey::BYTES == crypto_box_SECRETKEYBYTES as usize);
const_assert!(PublicKey::BYTES == crypto_box_PUBLICKEYBYTES as usize);
const_assert!(Nonce::BYTES == crypto_box_NONCEBYTES as usize);
const_assert!(KeyPair::SEED_BYTES == crypto_box_SEEDBYTES as usize);
const_assert!(CryptoBox::SYMMETRIC_KEY_BYTES == crypto_box_BEFORENMBYTES as usize);
const_assert!(CryptoBox::MAC_BYTES == crypto_box_MACBYTES as usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateKey([u8; Self::BYTES]);

impl TryFrom<&[u8]> for PrivateKey {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::BYTES {
            return Err(Error::Argument(format!(
                "Incorrect private key size {}, should be {}",
                bytes.len(),
                Self::BYTES
            )));
        }
        Ok(PrivateKey(bytes.try_into().unwrap()))
    }
}

impl TryFrom<&signature::PrivateKey> for PrivateKey {
    type Error = Error;
    fn try_from(sk: &signature::PrivateKey) -> Result<Self> {
        let mut bytes = [0u8; Self::BYTES];
        let rc = unsafe {
            crypto_sign_ed25519_sk_to_curve25519(
                as_uchar_ptr_mut!(bytes),
                as_uchar_ptr!(sk.as_bytes()),
            )
        };

        if rc != 0 {
            return Err(Error::Crypto(format!(
                "converts Ed25519 key to x25519 key failed."
            )))
        }
        Ok(PrivateKey(bytes))
    }
}

impl PrivateKey {
    pub const BYTES: usize = 32;

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        self.0.fill(0);
    }
}

impl Drop for PrivateKey {
    fn drop(&mut self) {
        self.clear();
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey(
    pub(crate) [u8; Self::BYTES]
);

impl TryFrom<&[u8]> for PublicKey {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::BYTES {
            return Err(Error::Argument(format!(
                "Incorrect public key size {}, expected {}",
                bytes.len(),
                Self::BYTES
            )));
        }
        Ok(PublicKey(bytes.try_into().unwrap()))
    }
}

impl TryFrom<&signature::PublicKey> for PublicKey {
    type Error = Error;
    fn try_from(pk: &signature::PublicKey) -> Result<Self> {
        let mut bytes = [0u8; Self::BYTES];
        let rc = unsafe {
            crypto_sign_ed25519_pk_to_curve25519(
                as_uchar_ptr_mut!(bytes),
                as_uchar_ptr!(pk.as_bytes()),
            )
        };

        if rc != 0 {
            return Err(Error::Crypto(format!(
                "converts Ed25519 key to x25519 key failed."
            )))
        }
        Ok(PublicKey(bytes))
    }
}

impl PublicKey {
    pub const BYTES: usize = 32;

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        self.0.fill(0);
    }
}

impl Drop for PublicKey {
    fn drop(&mut self) {
        self.clear();
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nonce([u8; Self::BYTES]);

impl TryFrom<&[u8]> for Nonce {
    type Error = Error;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::BYTES {
            return Err(Error::Argument(format!(
                "Incorrect nonce key size {}, expected {}",
                bytes.len(),
                Self::BYTES
            )));
        }
        Ok(Nonce(bytes.try_into().unwrap()))
    }
}

impl Nonce {
    pub const BYTES: usize = 24;

    pub fn random() -> Self {
        let mut bytes = [0u8; Self::BYTES];
        randomize_bytes(&mut bytes);
        Nonce(bytes)
    }

    pub fn increment(&mut self) -> &Self {
        unsafe {
            sodium_increment(
                as_uchar_ptr_mut!(self.0),
                Self::BYTES
            )
        }
        self
    }

    pub const fn size(&self) -> usize {
        Self::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        self.0.fill(0)
    }
}

impl Drop for Nonce {
    fn drop(&mut self) {
        self.clear();
    }
}

impl std::fmt::Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct KeyPair(PrivateKey, PublicKey);

impl TryFrom<&[u8]> for KeyPair {
    type Error = Error;
    fn try_from(sk: &[u8]) -> Result<Self> {
        if sk.len() != PrivateKey::BYTES {
            return Err(Error::Argument(
                format!("Invalid private key size {}, expected: {}",
                    sk.len(),
                    PrivateKey::BYTES
                )
            ));
        }

        let mut pk = [0u8; PublicKey::BYTES];
        unsafe {
            crypto_scalarmult_base(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr!(sk)
            );
        }

        Ok(KeyPair(
            PrivateKey::try_from(sk).unwrap(),
            PublicKey(pk)
        ))
    }
}

impl From<&PrivateKey> for KeyPair {
    fn from(sk: &PrivateKey) -> Self {
        let mut pk = [0u8; PublicKey::BYTES];

        unsafe {
            crypto_scalarmult_base(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr!(sk.as_bytes())
            );
        }

        KeyPair(
            sk.clone(),
            PublicKey(pk)
        )
    }
}

impl From<&signature::KeyPair> for KeyPair {
    fn from(kp: &signature::KeyPair) -> Self {
        let mut x25519 = [0u8; PrivateKey::BYTES];

        unsafe {
            crypto_sign_ed25519_sk_to_curve25519(
                as_uchar_ptr_mut!(x25519),
                as_uchar_ptr!(kp.private_key().as_bytes()),
            );
        }
        Self::try_from(x25519.as_slice()).unwrap()
    }
}

impl KeyPair {
    pub const SEED_BYTES: usize = 32;

    pub fn new() -> Self {
        let mut pk = [0u8; PublicKey::BYTES];
        let mut sk = [0u8; PrivateKey::BYTES];

        unsafe {
            crypto_box_keypair(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr_mut!(sk)
            );
        }

        KeyPair(PrivateKey(sk), PublicKey(pk))
    }

    pub fn random() -> Self {
        let mut seed = [0u8; KeyPair::SEED_BYTES];
        randomize_bytes(&mut seed);

        let mut sk = [0u8; PrivateKey::BYTES];
        let mut pk = [0u8; PublicKey::BYTES];

        unsafe { // Always success
            crypto_box_seed_keypair(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr_mut!(sk),
                as_uchar_ptr!(seed),
            );
        }

        KeyPair(PrivateKey(sk), PublicKey(pk))
    }

    pub fn try_from_seed(seed: &[u8]) -> Result<Self> {
        if seed.len() != KeyPair::SEED_BYTES {
            return Err(Error::Argument(format!(
                "Invalid seed size {}, should be {}",
                seed.len(),
                KeyPair::SEED_BYTES
            )));
        }

        let mut pk = [0u8; PublicKey::BYTES];
        let mut sk = [0u8; PrivateKey::BYTES];
        unsafe {
            crypto_box_seed_keypair(
                as_uchar_ptr_mut!(pk),
                as_uchar_ptr_mut!(sk),
                as_uchar_ptr!(seed),
            );
        }

        Ok(KeyPair(PrivateKey(sk), PublicKey(pk)))
    }

    pub const fn private_key(&self) -> &PrivateKey {
        &self.0
    }

    pub fn to_private_key(&self) -> PrivateKey {
        self.0.clone()
    }

    pub const fn public_key(&self) -> &PublicKey {
        &self.1
    }

    pub fn to_public_key(&self) -> PublicKey {
        self.1.clone()
    }

    pub fn clear(&mut self) {
        self.0.clear();
        self.1.clear();
    }
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        self.clear();
    }
}

#[derive(Debug)]
pub struct CryptoBox([u8; Self::SYMMETRIC_KEY_BYTES]);

impl TryFrom<(&PublicKey, &PrivateKey)> for CryptoBox {
    type Error = Error;
    fn try_from(kp: (&PublicKey, &PrivateKey)) -> Result<Self> {
        let mut key = [0u8; Self::SYMMETRIC_KEY_BYTES];
        let rc = unsafe {
            crypto_box_beforenm(
                as_uchar_ptr_mut!(key),
                as_uchar_ptr!(kp.0.as_bytes()),
                as_uchar_ptr!(kp.1.as_bytes()),
            )
        };
        if rc != 0 {
            return Err(Error::Crypto(format!(
                "Compute symmetric key failed, wrong public key or private key"
            )));
        }

        Ok(CryptoBox(key))
    }
}

impl CryptoBox {
    pub const SYMMETRIC_KEY_BYTES: usize = 32;
    pub const MAC_BYTES: usize = 16;

    pub const fn size(&self) -> usize {
        Self::SYMMETRIC_KEY_BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn clear(&mut self) {
        self.0.fill(0)
    }

    pub fn encrypt(&self,
        plain: &[u8],
        cipher: &mut [u8],
        nonce: &Nonce
    ) -> Result<usize> {
        let expected_len = plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES;
        if cipher.len() < expected_len {
            return Err(Error::Argument(format!("The input buffer is insufficient.")));
        }

        cipher[..Nonce::BYTES].copy_from_slice(nonce.as_bytes());
        let rc = unsafe {
            crypto_box_easy_afternm(
                as_uchar_ptr_mut!(cipher[Nonce::BYTES..expected_len]),
                as_uchar_ptr!(plain),
                plain.len() as libc::c_ulonglong,
                as_uchar_ptr!(cipher[..Nonce::BYTES]),
                as_uchar_ptr!(self.0),
            )
        };

        match rc == 0 {
            true => Ok(expected_len),
            false => return Err(Error::Crypto(format!("Data encryption failed")))
        }
    }

    pub fn encrypt_into(&self,
        plain: &[u8],
        nonce: &Nonce
    ) -> Result<Vec<u8>> {
        let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        self.encrypt(plain, cipher.as_mut(), nonce).map(|_| cipher)
    }

    pub fn decrypt(&self,
        cipher: &[u8],
        plain: &mut [u8],
        _nonce: &Nonce
    ) -> Result<usize> {
        let expected_len = cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES;
        if plain.len() < expected_len {
            return Err(Error::Argument(format!("The input buffer is insufficient.")));
        }

        let cipher_len = cipher.len() - Nonce::BYTES;
        //  Extract the nonce from the cipher text
        let rc = unsafe {
            crypto_box_open_easy_afternm(
                as_uchar_ptr_mut!(plain[.. expected_len]),
                as_uchar_ptr!(cipher[Nonce::BYTES..]),
                cipher_len as libc::c_ulonglong,
                as_uchar_ptr!(cipher[..Nonce::BYTES]),
                as_uchar_ptr!(self.0),
            )
        };

        match rc == 0 {
            true => Ok(expected_len),
            false => return Err(Error::Crypto(format!("Data decryption failed")))
        }
    }

    pub fn decrypt_into(&self,
        cipher: &[u8],
        nonce: &Nonce
    ) -> Result<Vec<u8>> {
        let mut plain = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        self.decrypt(cipher, plain.as_mut(), nonce).map(|_| plain)
    }
}

impl Drop for CryptoBox {
    fn drop(&mut self) {
        self.clear();
    }
}

pub fn encrypt(plain: &[u8],
    cipher: &mut [u8],
    nonce: &Nonce,
    pk: &PublicKey,
    sk: &PrivateKey,
) -> Result<usize> {
    let expected_len = plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES;
    if cipher.len() < expected_len {
        return Err(Error::Argument(format!("The input buffer is insufficient.")));
    }

    cipher[..Nonce::BYTES].copy_from_slice(nonce.as_bytes());
    let rc = unsafe {
        crypto_box_easy(
            as_uchar_ptr_mut!(cipher[Nonce::BYTES.. expected_len]),
            as_uchar_ptr!(plain),
            plain.len() as libc::c_ulonglong,
            as_uchar_ptr!(cipher[..Nonce::BYTES]),
            as_uchar_ptr!(pk.as_bytes()),
            as_uchar_ptr!(sk.as_bytes()),
        )
    };
    match rc == 0 {
        true => Ok(expected_len),
        false => return Err(Error::Crypto(format!("Data encryption failed")))
    }
}

pub fn encrypt_into(plain: &[u8],
    nonce: &Nonce,
    pk: &PublicKey,
    sk: &PrivateKey
) -> Result<Vec<u8>> {
    let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
    encrypt(plain, cipher.as_mut(), nonce, pk, sk).map(|_| cipher)
}

pub fn decrypt(cipher: &[u8],
    plain: &mut [u8],
    _nonce: &Nonce,
    pk: &PublicKey,
    sk: &PrivateKey,
) -> Result<usize> {
    let expected_len = cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES;
    if plain.len() < expected_len {
        return Err(Error::Argument(format!("The input buffer is insufficient.")));
    }

    let cipher_len = cipher.len() - Nonce::BYTES;
    //  Extract the nonce from the cipher text
    let rc = unsafe {
        crypto_box_open_easy(
            as_uchar_ptr_mut!(plain[..expected_len]),
            as_uchar_ptr!(cipher[Nonce::BYTES..]),
            cipher_len as libc::c_ulonglong,
            as_uchar_ptr!(cipher[..Nonce::BYTES]),
            as_uchar_ptr!(pk.as_bytes()),
            as_uchar_ptr!(sk.as_bytes()),
        )
    };

    match rc == 0 {
        true => Ok(expected_len),
        false => return Err(Error::Crypto(format!("Data decryption failed")))
    }
}

pub fn decrypt_into(cipher: &[u8],
    nonce: &Nonce,
    pk: &PublicKey,
    sk: &PrivateKey
) -> Result<Vec<u8>> {
    let mut plain = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
    decrypt(cipher, plain.as_mut(), nonce, pk, sk).map(|_| plain)
}
