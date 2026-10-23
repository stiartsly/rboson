//! Core Boson protocol primitives.

// Migrated code: it calls libsodium directly and is not documented yet. Remove these allows as the
// crate is brought in line with the workspace lints.
#![allow(unsafe_code)]
#![allow(missing_docs)]

pub mod crypto_identity;
pub mod errors;
pub mod id;
pub mod identity;
pub mod logger;
pub mod network;
pub mod node_info;
pub mod peer_info;
pub mod value;
pub mod version;

pub(crate) mod crypto_context;
pub(crate) mod crypto {
    pub mod cryptobox;
    pub mod signature;
    pub mod key_derivation;
    pub mod random;
}

pub use crate::{
    crypto::{cryptobox, key_derivation, random, signature},
    cryptobox::CryptoBox,
    crypto_identity::CryptoIdentity,
    errors::{Error, Result, *},
    id::{Id, DID_PREFIX},
    identity::Identity,
    network::Network,
    node_info::NodeInfo,
    peer_info::{PeerBuilder, PeerInfo},
    signature::Signature,
    value::{EncryptedBuilder, ImmutableBuilder, SignedBuilder, Value},
};

pub use crate::{
    crypto_context::CryptoContext,
};

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
pub mod utils {
    pub mod handler;
    pub mod promise;
    pub mod timer_client;
    pub mod timer_manager;
    pub mod codec;

    pub use codec::*;
}

#[allow(unused_imports)]
#[doc(hidden)]
pub use crate::utils::{
    handler::{BoxHandler, EasyHandler, LocalBoxHandler},
    promise::{Promise, PromiseFuture},
    timer_client::{BoxTimerClient, BoxTimerCmd, LocalBoxTimerClient, LocalBoxTimerCmd},
    timer_manager::{BoxTimerManager, LocalBoxTimerManager},
};

#[macro_export]
macro_rules! locked {
    ($mutex:expr) => {{
        $mutex.lock().unwrap()
    }};
}

#[macro_export]
macro_rules! elapsed_ms {
    ($time:expr) => {{
        $time
            .elapsed()
            .unwrap_or(std::time::Duration::MAX)
            .as_millis() as u128
    }};
}

#[macro_export]
macro_rules! as_ms {
    ($time:expr) => {{
        $time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }};
}

#[macro_export]
macro_rules! as_secs {
    ($time:expr) => {{
        $time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }};
}

#[macro_export]
macro_rules! unwrap {
    ($val:expr) => {{
        $val.as_ref().unwrap()
    }};
}

#[macro_export]
macro_rules! unwrap_mut {
    ($val:expr) => {{
        $val.as_mut().unwrap()
    }};
}

use std::net::IpAddr;
#[doc(hidden)]
pub fn local_addr(ipv4: bool) -> Result<IpAddr> {
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => return Err(e.into()),
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() && ((ipv4 && ip.is_ipv4()) || (!ipv4 && ip.is_ipv6())) {
            return Ok(ip);
        }
    }
    Err(NetworkError::new("No working network interfaces"))
}

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

#[allow(unused)]
#[doc(hidden)]
pub fn dump_hex(label: &str, data: &[u8]) {
    use hex::ToHex;
    let data_hex = data.encode_hex::<String>();
    println!("dumping(hex) {}: {}", label, data_hex);
}
