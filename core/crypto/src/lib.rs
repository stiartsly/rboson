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

// Migrated code: it calls libsodium directly and is not documented yet. Remove these allows as the
// crate is brought in line with the workspace lints.
#![allow(unsafe_code)]
#![allow(missing_docs)]

pub mod cryptobox;
pub mod key_derivation;
pub mod random;
pub mod signature;

pub mod errors {
    mod argument_error;
    mod crypto_error;

    pub type Error = Box<dyn std::error::Error>;
    pub type Result<T> = std::result::Result<T, Error>;

    pub use {argument_error::ArgumentError, crypto_error::CryptoError};
}

pub use errors::{Error, Result};

macro_rules! as_uchar_ptr {
    ($val:expr) => {
        $val.as_ptr().cast::<libc::c_uchar>()
    };
}
macro_rules! as_uchar_ptr_mut {
    ($val:expr) => {
        $val.as_mut_ptr().cast::<libc::c_uchar>()
    };
}

pub(crate) use {
    as_uchar_ptr,
    as_uchar_ptr_mut
};

#[doc(hidden)]
pub fn random_array<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    unsafe {
        libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut libc::c_void, N);
    };
    bytes
}

#[allow(unused)]
#[doc(hidden)]
pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    unsafe {
        libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut libc::c_void, len);
    };
    bytes
}
