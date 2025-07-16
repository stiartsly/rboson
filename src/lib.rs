pub mod core;
pub mod did;
pub mod dht;
pub mod appdata_store;
pub mod activeproxy;
pub mod messaging;

pub use crate::core::{
    id::{
        self,
        Id,
        DID_PREFIX,
        ID_BYTES,
        ID_BITS
    },

    error::{self, Error},
    prefix::{self, Prefix},
    signature::{self, Signature},
    cryptobox::{self, CryptoBox},
    node_info::{self, NodeInfo},
    peer_info::{self, PeerInfo, PeerBuilder},
    value::{
        self,
        Value,
        ValueBuilder,
        SignedBuilder,
        EncryptedBuilder
    },
    network::{self, Network},
    identity::{self, Identity},
    crypto_identity::{self, CryptoIdentity},
    crypto_context::{self, CryptoContext},
    joint_result::{self, JointResult},

    config,
    default_configuration as configuration,
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
    node::{self, Node}
};

pub use crate::activeproxy::{
    ActiveProxyClient
};

#[macro_export]
macro_rules! elapsed_ms {
    ($time:expr) => {{
        $time.elapsed().unwrap().as_millis() as u128
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
fn local_addr(ipv4: bool) -> crate::core::Result<IpAddr>{
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => return Err(Error::from(e))
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() &&
            ((ipv4 && ip.is_ipv4()) ||
            (!ipv4 && ip.is_ipv6())) {
            return Ok(ip)
        }
    }
    Err(Error::Network(format!("No working network interfaces")))
}

fn create_dirs(input: &str) -> crate::core::Result<()> {
    let path = std::path::Path::new(input);
    if path.exists() {
        return Ok(())
    }

    std::fs::create_dir_all(path).map_err(|e|
         Error::Io(format!("Creating directory path {} error: {e}", input))
    )
}

fn randomize_bytes<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        libsodium_sys::randombytes_buf(
            array.as_mut_ptr() as *mut libc::c_void,
            N
        );
    }
}

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

pub(crate) fn is_none_or_empty<T: IsEmpty>(v: &Option<T>) -> bool {
    v.as_ref().map(|s| s.is_empty()).unwrap_or(true)
}

trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for String {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for u64 {
    fn is_empty(&self) -> bool {
        *self == 0
    }
}
