pub mod core;
pub mod activeproxy;
pub mod messaging;

pub use {
    core::id,
    core::id::Id,
    core::node::Node,
    core::error::Error,
    core::error,
    core::config,
    core::config::Config,
    core::prefix::Prefix,
    core::node_info::NodeInfo,
    core::peer_info::{
        PeerInfo,
        PeerBuilder
    },
    core::value::{
        Value,
        ValueBuilder,
        SignedBuilder,
        EncryptedBuilder
    },
    core::network::Network,
    core::node_status::NodeStatus,
    core::lookup_option::LookupOption,
    core::joint_result::JointResult,
    core::default_configuration as configuration,
    core::signature,
    core::signature::Signature,
    core::cryptobox,
    core::cryptobox::CryptoBox,

    activeproxy::ActiveProxyClient,
};

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

#[macro_export]
macro_rules! unwrap_mut {
    ($val:expr) => {{
        $val.as_mut().unwrap()
    }};
}

use std::fs;
use std::net::IpAddr;
use std::path::Path;
use get_if_addrs::get_if_addrs;
use libsodium_sys::randombytes_buf;

fn local_addr(ipv4: bool) -> Result<IpAddr, Error>{
    let if_addrs = match get_if_addrs() {
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

#[allow(dead_code)]
fn random_bytes(len: usize) -> Vec<u8> {
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
