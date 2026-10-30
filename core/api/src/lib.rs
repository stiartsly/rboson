// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Core Boson protocol primitives.

// Migrated code that is not documented yet. Remove this allow as the crate is brought in line
// with the workspace lints.
#![allow(missing_docs)]

pub mod crypto_identity;
pub mod errors;
pub mod id;
pub mod identity;
pub mod logger;
pub mod network;
pub mod node_info;
pub mod peer_info;
pub mod value;
pub mod version;

pub(crate) mod crypto_context;

pub use boson_crypto::{cryptobox, key_derivation, random, signature};

pub use crate::{
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

pub use crate::{
    crypto_context::CryptoContext,
};


#[doc(hidden)]
pub mod utils {
    pub mod handler;
    pub mod promise;
    pub mod timer_client;
    pub mod timer_manager;
    pub mod codec;

    pub use codec::*;
}

#[allow(unused_imports)]
#[doc(hidden)]
pub use crate::utils::{
    handler::{BoxHandler, EasyHandler, LocalBoxHandler},
    promise::{Promise, PromiseFuture},
    timer_client::{BoxTimerClient, BoxTimerCmd, LocalBoxTimerClient, LocalBoxTimerCmd},
    timer_manager::{BoxTimerManager, LocalBoxTimerManager},
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
        $time
            .elapsed()
            .unwrap_or(std::time::Duration::MAX)
            .as_millis() as u128
    }};
}

#[macro_export]
macro_rules! as_ms {
    ($time:expr) => {{
        $time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }};
}

#[macro_export]
macro_rules! as_secs {
    ($time:expr) => {{
        $time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
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
#[doc(hidden)]
pub fn local_addr(ipv4: bool) -> Result<IpAddr> {
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => return Err(e.into()),
    };

    for iface in if_addrs {
        let name = iface.name.to_ascii_lowercase();
        if ["utun", "tun", "tap", "ppp", "ipsec", "wg"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            continue;
        }
        let ip = iface.ip();
        if !ip.is_loopback() && ((ipv4 && ip.is_ipv4()) || (!ipv4 && ip.is_ipv6())) {
            return Ok(ip);
        }
    }
    Err(NetworkError::new("No working network interfaces"))
}

#[doc(hidden)]
pub use boson_crypto::{random_array, random_bytes};

#[allow(unused)]
#[doc(hidden)]
pub fn dump_hex(label: &str, data: &[u8]) {
    use hex::ToHex;
    let data_hex = data.encode_hex::<String>();
    println!("dumping(hex) {}: {}", label, data_hex);
}
