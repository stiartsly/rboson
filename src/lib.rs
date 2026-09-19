pub mod activeproxy;
pub mod core;
pub mod dht;
pub mod did;
pub mod director;
pub mod messaging;

pub(crate) mod utils {
    pub(crate) mod handler;
    pub(crate) mod promise;
    pub(crate) mod timer_client;
    pub(crate) mod timer_manager;
    pub(crate) mod utils;

    pub(crate) use utils::*;
}

#[allow(unused_imports)]
pub(crate) use crate::utils::{
    handler::{BoxHandler, EasyHandler, LocalBoxHandler},
    promise::{Promise, PromiseFuture},
    timer_client::{BoxTimerClient, BoxTimerCmd, LocalBoxTimerClient, LocalBoxTimerCmd},
    timer_manager::{BoxTimerManager, LocalBoxTimerManager},
};

pub(crate) use crate::core::{
    crypto_context::{CryptoContext},
};

pub use crate::core::{
    cryptobox::{self, CryptoBox},
    errors::{self, Error, Result},
    id::{self, Id, DID_PREFIX},
    identity::{self, CryptoIdentity, Identity},
    network::{self, Network},
    node_info::{self, NodeInfo},
    peer_info::{self, PeerBuilder, PeerInfo},
    signature::{self, Signature},
    value::{self, EncryptedBuilder, ImmutableBuilder, SignedBuilder, Value},
};

pub use crate::did::{
    cached_resolver, card, card_builder, credential, credential_builder, dht_registry,
    dht_resolver, didurl, filesystem_resolution_cache, proof, registry, resolution_cache,
    resolution_errors, resolver, verification_method, vouch, vouch_builder, w3c,
};

pub use crate::dht::{
    connection_status::{self, ConnectionStatus},
    connection_status_listener::{self, ConnectionStatusListener},
    node::{self, Node},
};

pub use crate::activeproxy::{Client, Options};

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

use crate::errors::NetworkError;
use std::net::IpAddr;
fn local_addr(ipv4: bool) -> Result<IpAddr> {
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

fn random_array<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    unsafe {
        libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut libc::c_void, N);
    };
    bytes
}

#[allow(unused)]
fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    unsafe {
        libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut libc::c_void, len);
    };
    bytes
}

#[allow(unused)]
fn dump_hex(label: &str, data: &[u8]) {
    use hex::ToHex;
    let data_hex = data.encode_hex::<String>();
    println!("dumping(hex) {}: {}", label, data_hex);
}
