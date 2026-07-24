pub(crate) mod logger;
pub(crate) mod version;

pub mod config;
pub mod id;
pub mod joint_result;
pub mod network;
pub mod identity;
pub mod crypto_context;
pub mod signature;
pub mod cryptobox;
pub mod node_info;
pub mod peer_info;
pub mod value;
pub mod errors;

pub use crate::core::{
    id::{Id, DID_PREFIX},
    errors::{Error, Result},

    identity::{Identity, CryptoIdentity},
    crypto_context::CryptoContext,

    signature::Signature,
    cryptobox::CryptoBox,

    joint_result::JointResult,
    network::Network,
    config::{
        Config,
        UserConfig,
        DeviceConfig,
        ActiveProxyConfig,
        MessagingConfig,
    },
    node_info::NodeInfo,
    peer_info::{PeerInfo, PeerBuilder},
    value::{Value, ImmutableBuilder, SignedBuilder, EncryptedBuilder},
};

#[cfg(test)]
mod unitests {
    mod test_id;
    mod test_logger;
    mod test_version;
    mod test_value;
    mod test_node_info;
    mod test_peer_info;
    mod test_crypto_identity;
    mod test_crypto_context;
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
