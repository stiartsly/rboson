use super::{
    Id,
    CryptoContext,
    Result,
};

/// An entity capable of signing, verifying, encrypting, and decrypting.
pub trait Identity {
    /// Returns the identifier of this identity.
    fn id(&self) -> &Id;

    /// Signs `data` with this identity's private signing key.
    ///
    /// Writes the 64-byte Ed25519 signature to `sig` and returns the number of
    /// bytes written.
    fn sign(&self, data: &[u8], sig: &mut [u8]) -> Result<usize>;

    /// Verifies that `sig` is a valid signature over `data` for this identity.
    ///
    /// Returns `Ok(true)` for a valid signature and `Ok(false)` for an invalid
    /// signature.
    fn verify(&self, data: &[u8], sig: &[u8]) -> Result<bool>;

    /// Signs `data` and returns the 64-byte Ed25519 signature in a new vector.
    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>>;

    /// Encrypts `plain` for `recipient` using this identity's private encryption key.
    ///
    /// Prepends a random nonce to `cipher` and returns the number of bytes written.
    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize>;

    /// Decrypts `cipher` received from `sender` into `plain`.
    ///
    /// The ciphertext must begin with the nonce written by [`Identity::encrypt`].
    /// Returns the number of plaintext bytes written.
    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize>;

    /// Encrypts `plain` for `rec` and returns nonce-prefixed ciphertext in a new vector.
    fn encrypt_into(&self, rec: &Id, plain: &[u8]) -> Result<Vec<u8>>;

    /// Decrypts nonce-prefixed `cipher` from `sender` and returns the plaintext in a new vector.
    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>>;

    fn encrypt_with_nonce(
        &self,
        recipient: &Id,
        nonce: &[u8],
        plain: &[u8],
        cipher: &mut [u8]
    ) -> Result<usize>;

    /// Decrypts raw ciphertext produced by [`Identity::encrypt_with_nonce`].
    ///
    /// `cipher` must not contain a nonce prefix; pass the nonce explicitly.
    fn decrypt_with_nonce(
        &self,
        sender: &Id,
        nonce: &[u8],
        cipher: &[u8],
        plain: &mut [u8]
    ) -> Result<usize>;

    /// Encrypts with a supplied nonce and returns raw ciphertext in a new vector.
    fn encrypt_with_nonce_into(
        &self,
        rec: &Id,
        nonce: &[u8],
        plain: &[u8]
    ) -> Result<Vec<u8>>;

    /// Decrypts raw ciphertext with a supplied nonce and returns the plaintext.
    fn decrypt_with_nonce_into(
        &self,
        sender: &Id,
        nonce: &[u8],
        cipher: &[u8]
    ) -> Result<Vec<u8>>;

    /// Creates a reusable encryption context for communication with the identity `id`.
    ///
    /// The resulting context combines this identity's private encryption key with
    /// the peer's public encryption key.
    fn create_crypto_context(&self, _id: &Id) -> Result<CryptoContext>;
}
