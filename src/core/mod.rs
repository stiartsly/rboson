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
