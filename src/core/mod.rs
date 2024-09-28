mod constants;
mod crypto_cache;
mod dht;
mod kbucket;
mod kclosest_nodes;
mod logger;
mod msg;
mod server;
mod rpccall;
mod task;
mod scheduler;
mod sqlite3;

pub(crate) mod data_storage;
pub(crate) mod kbucket_entry;
pub(crate) mod routing_table;
pub(crate) mod sqlite_storage;
pub(crate) mod token_manager;
pub(crate) mod node_runner;
pub(crate) mod bootstrap_channel;
pub(crate) mod version;
pub(crate) mod future;

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
macro_rules! addr_family {
    ($val:expr) => {{
        match $val.is_ipv4() {
            true => "ipv4",
            false => "ipv6"
        }
    }};
}

use std::net::{
    SocketAddr,
    IpAddr
};

fn is_broadcast(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_broadcast(),
        IpAddr::V6(_) => false
    }
}

fn is_linklocal(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => {
            let v = &v6.octets();
            v[0] == 0xfe && v[1] == 0x80
        }
    }
}

fn is_sitelocal(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if !v4.is_private() {
                return false;
            }

            let v = &v4.octets();
            let b1 = v[0];
            let b2 = v[1];

            // 10.0.0.0/8
            if b1 == 10 {
                return true;
            }

            // 172.16.0.0/12
            if (b1 == 172) && (b2 >= 16) && (b2 <= 31) {
                return true;
            }

            // 192.168.0.0/16
            if (b1 == 192) && (b2 == 168) {
                return true;
            }
            false
        },
        IpAddr::V6(v6) => {
            let v = &v6.octets();
            v[0] == 0xfc || v[0] == 0xfd
        }
    }
}

fn is_mapped_ipv4(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(_) => return false,
        IpAddr::V6(v6) => {
            let mapped_ipv4_prefix = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
            let octets = v6.octets().to_vec();
            octets == mapped_ipv4_prefix
        }
    }
}

pub(crate) fn is_global_unicast(ip: &IpAddr) -> bool {
    !(ip.is_loopback() ||
        ip.is_multicast() ||
        ip.is_unspecified() ||
        is_broadcast(ip) ||
        is_linklocal(ip) ||
        is_sitelocal(ip) ||
        is_mapped_ipv4(ip))
}

pub(crate) fn is_any_unicast(ip: &IpAddr) -> bool {
    is_global_unicast(ip) || is_sitelocal(ip)
}

pub(crate) fn is_bogon(addr: &SocketAddr) -> bool {
   !(addr.port() > 0 &&
     addr.port() < 0xFFFF && is_global_unicast(&addr.ip()))
}
