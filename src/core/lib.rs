mod constants;
mod crypto_cache;
mod data_storage;
mod dht;
mod kbucket;
mod kbucket_entry;
mod kclosest_nodes;
mod logger;
mod msg;
mod server;
mod routing_table;
mod rpccall;
mod sqlite_storage;
mod task;
mod token_manager;
mod version;
mod scheduler;
mod node_runner;
mod bootstrap_channel;
mod future;
mod sqlite3;

pub mod id;
pub mod config;
pub mod cryptobox;
pub mod default_configuration;
pub mod error;
pub mod lookup_option;
pub mod node_info;
pub mod node_status;
pub mod peer_info;
pub mod prefix;
pub mod joint_result;
pub mod network;
pub mod node;
pub mod signature;
pub mod value;

#[cfg(test)] mod test_id;
#[cfg(test)] mod test_peer_info;
#[cfg(test)] mod test_node_info;
#[cfg(test)] mod test_value;
#[cfg(test)] mod test_sqlite_storage;
#[cfg(test)] mod test_token_man;
#[cfg(test)] mod test_routing_table;
#[cfg(test)] mod test_node_runner;

pub use {
    id::Id,
    node::Node,
    error::Error,
    config::Config,
    prefix::Prefix,
    network::Network,
    node_info::NodeInfo,
    peer_info::{PeerInfo, PeerBuilder},
    value::{Value, ValueBuilder, SignedBuilder, EncryptedBuilder},
    node_status::NodeStatus,
    lookup_option::LookupOption,
    joint_result::JointResult,
    signature::Signature,
    cryptobox::CryptoBox,
};

pub(crate) use {
    bootstrap_channel   as bootstr,
    sqlite_storage      as sqlite,
    token_manager       as token,
    data_storage        as storage,
    crypto_cache        as crypto,
};

pub fn distance(a: &Id, b: &Id) -> Id {
    a.distance(b)
}

#[macro_export]
macro_rules! as_uchar_ptr {
    ($val:expr) => {{
        $val.as_ptr() as *const libc::c_uchar
    }};
}

#[macro_export]
macro_rules! as_uchar_ptr_mut {
    ($val:expr) => {{
        $val.as_mut_ptr() as *mut libc::c_uchar
    }};
}

#[macro_export]
macro_rules! is_bogon_addr {
    ($val:expr) => {{
        // TODO:
        false
    }};
}

#[macro_export]
macro_rules! addr_family {
    ($val:expr) => {{
        match $val.is_ipv4() {
            true => "ipv4",
            false => "ipv6"
        }
    }};
}

#[macro_export]
macro_rules! as_millis {
    ($time:expr) => {{
        $time.elapsed().unwrap().as_millis()
    }};
}

#[macro_export]
macro_rules! unwrap {
    ($val:expr) => {{
        $val.as_ref().unwrap()
    }};
}

use std::fs;
use std::net::IpAddr;
use std::path::Path;
use get_if_addrs::get_if_addrs;

#[cfg(test)]
use std::env;
#[cfg(test)]
use libsodium_sys::randombytes_buf;

fn local_addr(ipv4: bool) -> Option<IpAddr>{
    let if_addrs = match get_if_addrs() {
        Ok(v) => v,
        _ => return None
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() &&
            ((ipv4 && ip.is_ipv4()) ||
            (!ipv4 && ip.is_ipv6())) {
            return Some(ip)
        }
    }
    None
}

fn create_dirs(input: &str) -> Result<(), Error> {
    let path = Path::new(input);
    if path.exists() {
        return Ok(())
    }

    fs::create_dir_all(path).map_err(|e|
         Error::Io(format!("Creating directory path {} error: {}", input, e))
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

#[cfg(test)]
fn create_random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    unsafe {
        randombytes_buf(
            bytes.as_mut_ptr() as *mut libc::c_void,
            len
        );
        bytes.set_len(len);
    };
    bytes
}

#[cfg(test)]
fn working_path(input: &str) -> String {
    let path = env::current_dir().unwrap().join(input);
    if !fs::metadata(&path).is_ok() {
        match fs::create_dir(&path) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create directory: {}", e);
            }
        }
    }
    path.display().to_string()
}

#[cfg(test)]
fn remove_working_path(input: &str) {
    if fs::metadata(&input).is_ok() {
        match fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}
