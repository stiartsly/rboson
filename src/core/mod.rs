pub mod logger;
pub(crate) mod version;

pub mod crypto_context;
pub mod cryptobox;
pub mod errors;
pub mod id;
pub mod identity;
pub mod network;
pub mod node_info;
pub mod peer_info;
pub mod signature;
pub mod value;

pub use crate::core::{
    crypto_context::CryptoContext,
    cryptobox::CryptoBox,
    errors::{Error, Result},
    id::{Id, DID_PREFIX},
    identity::{CryptoIdentity, Identity},
    network::Network,
    node_info::NodeInfo,
    peer_info::{PeerBuilder, PeerInfo},
    signature::Signature,
    value::{EncryptedBuilder, ImmutableBuilder, SignedBuilder, Value},
};

#[cfg(test)]
mod unitests {
    mod test_crypto_context;
    mod test_crypto_identity;
    mod test_id;
    mod test_logger;
    mod test_node_info;
    mod test_peer_info;
    mod test_value;
    mod test_version;
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
