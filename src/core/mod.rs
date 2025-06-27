pub(crate) mod logger;
pub(crate) mod cbor;
pub(crate) mod version;

pub mod config;
pub mod default_configuration;

pub mod id;
pub mod prefix;

pub mod error;
pub mod joint_result;
pub mod network;

pub mod identity;
pub mod crypto_identity;
pub mod crypto_context;

pub mod signature;
pub mod cryptobox;

pub mod node_info;
pub mod peer_info;
pub mod value;

pub use crate::core::{
    id::{
        Id,
        DID_PREFIX,
        ID_BYTES,
        ID_BITS
    },
    prefix::Prefix,

    identity::Identity,
    crypto_identity::CryptoIdentity,
    crypto_context::CryptoContext,

    signature::Signature,
    cryptobox::CryptoBox,

    error::{Error, Result},
    joint_result::JointResult,
    network::Network,
    node_info::NodeInfo,
    peer_info::{PeerInfo, PeerBuilder},
    value::{
        Value,
        ValueBuilder,
        SignedBuilder,
        EncryptedBuilder
    },
};

#[cfg(test)]
mod unitests {
    mod test_id;
    mod test_logger;
    mod test_version;
    mod test_value;
    mod test_node_info;
    mod test_peer_info;

    #[allow(non_upper_case_globals)]
    static create_random_bytes: fn(usize) -> Vec<u8> = crate::random_bytes;
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
