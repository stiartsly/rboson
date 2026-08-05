pub mod core;
pub mod did;
pub mod dht;
pub mod activeproxy;
pub mod messaging;
pub(crate) mod utils;

pub use crate::core::{
    id::{
        self,
        Id,
        DID_PREFIX,
    },

    errors::{self, Error, Result},
    signature::{self, Signature},
    cryptobox::{self, CryptoBox},
    node_info::{self, NodeInfo},
    peer_info::{self, PeerInfo, PeerBuilder},
    value::{
        self,
        Value,
        ImmutableBuilder,
        SignedBuilder,
        EncryptedBuilder
    },
    network::{self, Network},
    identity::{self, Identity, CryptoIdentity},
    crypto_context::{self, CryptoContext},

    //node_config::{self, NodeConfig},
    //default_configuration as configuration,
};

pub use crate::did::{
    didurl,
    verification_method,
    proof,
    w3c,
    credential,
    credential_builder,
    vouch,
    vouch_builder,
    card,
    card_builder,
};

pub use crate::dht::{
    node::{self, Node},
    connection_status::{self, ConnectionStatus},
    connection_status_listener::{self, ConnectionStatusListener},
    configuration as node_configuration,
};

pub use crate::activeproxy::{
    ActiveProxyClient
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
        $time.elapsed().unwrap_or(std::time::Duration::MAX).as_millis() as u128
    }};
}

#[macro_export]
macro_rules! as_ms {
    ($time:expr) => {{
        $time.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
    }};
}

#[macro_export]
macro_rules! as_secs {
    ($time:expr) => {{
        $time.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
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
use crate::errors::NetworkError;
fn local_addr(ipv4: bool) -> Result<IpAddr>{
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => return Err(e.into()),
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() &&
            ((ipv4 && ip.is_ipv4()) ||
            (!ipv4 && ip.is_ipv6())) {
            return Ok(ip)
        }
    }
    Err(NetworkError::new("No working network interfaces"))
}

fn random_array<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    unsafe {
        libsodium_sys::randombytes_buf(
            bytes.as_mut_ptr() as *mut libc::c_void,
            N
        );
    };
    bytes
}

#[allow(unused)]
fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    unsafe {
        libsodium_sys::randombytes_buf(
            bytes.as_mut_ptr() as *mut libc::c_void,
            len
        );
    };
    bytes
}

#[allow(unused)]
fn dump_hex(label: &str, data: &[u8]) {
    use hex::ToHex;
    let data_hex = data.encode_hex::<String>();
    println!("dumping(hex) {}: {}", label, data_hex);
}
