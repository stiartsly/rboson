// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Boson cryptography: Ed25519 signatures, X25519 key agreement, crypto box, secret streams,
//! hashing, key derivation, password hashing, key backup and registration proof of work.

// Migrated code that is not documented yet. Remove this allow as the crate is brought in line
// with the workspace lints.
#![allow(missing_docs)]

pub mod cryptobox;
pub mod key_derivation;
pub mod random;
pub mod signature;

#[cfg(test)]
mod unitests {
    mod test_cryptobox;
    mod test_key_derivation;
    mod test_random;
    mod test_signature;
}

pub mod errors {
    mod argument_error;
    mod crypto_error;

    pub type Error = Box<dyn std::error::Error>;
    pub type Result<T> = std::result::Result<T, Error>;

    pub use {argument_error::ArgumentError, crypto_error::CryptoError};
}

pub use errors::{Error, Result};

/// Initializes libsodium. The functions of this crate that use its random generator call it
/// themselves, so applications need not; calling it early only moves the one-time cost.
pub fn init() -> Result<()> {
    static INIT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*INIT.get_or_init(|| libsodium_rs::ensure_init().is_ok()) {
        return Err(errors::CryptoError::new("libsodium initialization failed"));
    }
    Ok(())
}

/// Initializes libsodium, or panics when it cannot be initialized.
pub(crate) fn ensure_init() {
    init().expect("libsodium initialization failed");
}

/// Wipes `bytes` in a way the compiler cannot optimize away.
pub(crate) fn memzero(bytes: &mut [u8]) {
    libsodium_rs::utils::memzero(bytes);
}

#[doc(hidden)]
pub fn random_array<const N: usize>() -> [u8; N] {
    ensure_init();
    let mut bytes = [0u8; N];
    libsodium_rs::random::fill_bytes(&mut bytes);
    bytes
}

#[allow(unused)]
#[doc(hidden)]
pub fn random_bytes(len: usize) -> Vec<u8> {
    ensure_init();
    let mut bytes = vec![0u8; len];
    libsodium_rs::random::fill_bytes(&mut bytes);
    bytes
}
