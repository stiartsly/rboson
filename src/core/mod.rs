//! Core Boson protocol primitives.

pub mod crypto_identity;
pub mod errors;
pub mod id;
pub mod identity;
pub mod logger;
pub mod network;
pub mod node_info;
pub mod peer_info;
pub mod value;

pub(crate) mod crypto_context;
pub(crate) mod crypto {
    pub mod cryptobox;
    pub mod signature;
    pub mod key_derivation;
    pub mod random;
}

pub use crate::core::{
    crypto::{cryptobox, key_derivation, random, signature},
    cryptobox::CryptoBox,
    crypto_identity::CryptoIdentity,
    errors::{Error, Result, *},
    id::{Id, DID_PREFIX},
    identity::Identity,
    network::Network,
    node_info::NodeInfo,
    peer_info::{PeerBuilder, PeerInfo},
    signature::Signature,
    value::{EncryptedBuilder, ImmutableBuilder, SignedBuilder, Value},
};

pub(crate) use crate::core::{
    crypto_context::CryptoContext,
};

macro_rules! as_uchar_ptr {
    ($val:expr) => {
        $val.as_ptr().cast::<libc::c_uchar>()
    };
}
macro_rules! as_uchar_ptr_mut {
    ($val:expr) => {
        $val.as_mut_ptr().cast::<libc::c_uchar>()
    };
}

pub(crate) use {
    as_uchar_ptr,
    as_uchar_ptr_mut
};
