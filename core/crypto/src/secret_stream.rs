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

//! Authenticated encryption of a stream of messages, as the Java SDK's `SecretStream`: libsodium's
//! `crypto_secretstream_xchacha20poly1305`.
//!
//! An [`EncryptionStream`] encrypts a sequence of messages under one key; its [`header`] must reach
//! the receiver, whose [`DecryptionStream`] decrypts them in the same order. Each message can carry
//! additional authenticated data, and the last one is marked as final, so a truncated stream is
//! detected.
//!
//! [`header`]: EncryptionStream::header

use libsodium_rs::crypto_secretstream::xchacha20poly1305 as stream;
use static_assertions::const_assert;

use crate::{
    errors::{ArgumentError, CryptoError},
    Result,
};

/// The size of a stream key.
pub const KEY_BYTES: usize = 32;
/// The size of the header that starts a stream.
pub const HEADER_BYTES: usize = 24;
/// The bytes each encrypted message adds: its tag byte and its authentication tag.
pub const ABYTES: usize = 17;

const_assert!(KEY_BYTES == stream::KEYBYTES);
const_assert!(HEADER_BYTES == stream::HEADERBYTES);
const_assert!(ABYTES == stream::ABYTES);

/// Starts encrypting a stream with `key`.
pub fn encryption_stream(key: &[u8]) -> Result<EncryptionStream> {
    EncryptionStream::new(key)
}

/// Starts decrypting a stream that begins with `header`, with `key`.
pub fn decryption_stream(header: &[u8], key: &[u8]) -> Result<DecryptionStream> {
    DecryptionStream::new(header, key)
}

/// Encrypts the messages of a stream, in order.
pub struct EncryptionStream {
    state: stream::PushState,
    header: [u8; HEADER_BYTES],
    complete: bool,
}

impl EncryptionStream {
    /// Starts encrypting a stream with `key`, of [`KEY_BYTES`].
    pub fn new(key: &[u8]) -> Result<Self> {
        let key = stream_key(key)?;
        crate::ensure_init();
        let (state, header) = stream::PushState::init_push(&key)
            .map_err(|e| CryptoError::new(format!("Starting the stream failed: {e}")))?;
        Ok(Self {
            state,
            header,
            complete: false,
        })
    }

    /// The header that starts the stream. The receiver needs it to decrypt.
    pub fn header(&self) -> &[u8] {
        &self.header
    }

    /// Encrypts a message that is not the last one, with optional additional authenticated data.
    pub fn push(&mut self, message: &[u8], additional: Option<&[u8]>) -> Result<Vec<u8>> {
        self.push_tagged(message, additional, false)
    }

    /// Encrypts the last message of the stream, with optional additional authenticated data. The
    /// stream is complete afterwards.
    pub fn push_last(&mut self, message: &[u8], additional: Option<&[u8]>) -> Result<Vec<u8>> {
        self.push_tagged(message, additional, true)
    }

    /// Whether the last message has been encrypted.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    fn push_tagged(&mut self, message: &[u8], additional: Option<&[u8]>, last: bool) -> Result<Vec<u8>> {
        if self.complete {
            return Err(CryptoError::new("The stream is already complete"));
        }
        let tag = if last { stream::TAG_FINAL } else { stream::TAG_MESSAGE };
        let cipher = self
            .state
            .push(message, additional, tag)
            .map_err(|e| CryptoError::new(format!("Stream encryption failed: {e}")))?;
        self.complete = last;
        Ok(cipher)
    }
}

/// Decrypts the messages of a stream, in the order they were encrypted.
pub struct DecryptionStream {
    state: stream::PullState,
    complete: bool,
}

impl DecryptionStream {
    /// Starts decrypting a stream that begins with `header`, of [`HEADER_BYTES`], with `key`, of
    /// [`KEY_BYTES`].
    pub fn new(header: &[u8], key: &[u8]) -> Result<Self> {
        let header: &[u8; HEADER_BYTES] = header.try_into().map_err(|_| {
            ArgumentError::new(format!(
                "Invalid stream header size {}, expected {HEADER_BYTES}",
                header.len()
            ))
        })?;
        let key = stream_key(key)?;
        crate::ensure_init();
        let state = stream::PullState::init_pull(header, &key)
            .map_err(|_| CryptoError::new("Invalid stream header"))?;
        Ok(Self {
            state,
            complete: false,
        })
    }

    /// Decrypts the next message, with the additional authenticated data it was encrypted with. The
    /// stream is complete after its last message.
    pub fn pull(&mut self, cipher: &[u8], additional: Option<&[u8]>) -> Result<Vec<u8>> {
        if cipher.len() < ABYTES {
            return Err(ArgumentError::new(format!(
                "Stream message length {} is smaller than the overhead {ABYTES}",
                cipher.len()
            )));
        }
        if self.complete {
            return Err(CryptoError::new("The stream is already complete"));
        }
        let (message, tag) = self
            .state
            .pull(cipher, additional)
            .map_err(|_| CryptoError::new("Stream decryption failed"))?;
        if tag == stream::TAG_FINAL {
            self.complete = true;
        }
        Ok(message)
    }

    /// Whether the last message has been decrypted.
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

fn stream_key(key: &[u8]) -> Result<stream::Key> {
    if key.len() != KEY_BYTES {
        return Err(ArgumentError::new(format!(
            "Invalid stream key size {}, expected {KEY_BYTES}",
            key.len()
        )));
    }
    stream::Key::from_bytes(key).map_err(|e| CryptoError::new(e.to_string()).into())
}
