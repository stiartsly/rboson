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

use libsodium_rs::crypto_kdf;
use sha2::{Digest, Sha256};

use crate::errors::{ArgumentError, CryptoError, Result};

pub const MASTER_KEY_BYTES: usize = 32;
pub const CONTEXT_BYTES: usize = 8;
pub const MIN_SUBKEY_BYTES: usize = 16;
pub const MAX_SUBKEY_BYTES: usize = 64;

/// Derives a subkey with libsodium's `crypto_kdf` construction.
pub fn derive_key(
    master_key: &[u8],
    sub_key_id: u64,
    context: &[u8],
    sub_key_length: usize,
) -> Result<Vec<u8>> {
    if master_key.len() != MASTER_KEY_BYTES {
        return Err(ArgumentError::new(format!(
            "Invalid master key size: expected {MASTER_KEY_BYTES} bytes, got {}",
            master_key.len()
        ))
        .into());
    }
    if context.len() != CONTEXT_BYTES {
        return Err(ArgumentError::new(format!(
            "Invalid context size: expected {CONTEXT_BYTES} bytes, got {}",
            context.len()
        ))
        .into());
    }
    check_subkey_length(sub_key_length)?;

    let key = crypto_kdf::Key::from_slice(master_key)
        .map_err(|_| CryptoError::new("Key derivation failed"))?;
    crypto_kdf::derive_from_key(sub_key_length, sub_key_id, context, &key)
        .map_err(|_| CryptoError::new("Key derivation failed").into())
}

/// Reduces a context string to the fixed 8-byte KDF context, as Java does.
pub fn context_bytes(context: &str) -> Result<[u8; CONTEXT_BYTES]> {
    if context.is_empty() {
        return Err(ArgumentError::new("context must not be empty").into());
    }
    let digest = Sha256::digest(context.as_bytes());
    let mut output = [0u8; CONTEXT_BYTES];
    for (index, byte) in digest.iter().enumerate() {
        output[index % CONTEXT_BYTES] = output[index % CONTEXT_BYTES].wrapping_add(*byte);
    }
    Ok(output)
}

/// Derives a subkey from a string context.
pub fn derive_key_from_context(
    master_key: &[u8],
    sub_key_id: u64,
    context: &str,
    sub_key_length: usize,
) -> Result<Vec<u8>> {
    derive_key(
        master_key,
        sub_key_id,
        &context_bytes(context)?,
        sub_key_length,
    )
}

fn check_subkey_length(sub_key_length: usize) -> Result<()> {
    if !(MIN_SUBKEY_BYTES..=MAX_SUBKEY_BYTES).contains(&sub_key_length) {
        return Err(ArgumentError::new(format!(
            "Invalid subkey size: expected {MIN_SUBKEY_BYTES} to {MAX_SUBKEY_BYTES} bytes, got {sub_key_length}"
        ))
        .into());
    }
    Ok(())
}
