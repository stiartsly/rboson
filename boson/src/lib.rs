/*
 * Copyright (c) 2023 -      bosonnetwork.io
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

//! Boson network for Rust.
//!
//! Re-exports the Boson crates under short paths. The node implementations, the client
//! libraries and the shared HTTPS transport are behind features; `full` enables all of them.
//! The clients that talk HTTPS enable `http` themselves.

pub use boson_api as api;
pub use boson_crypto as crypto;

#[cfg(feature = "identifier")]
pub use boson_identifier as identifier;

#[cfg(feature = "http")]
pub use boson_http as http;

#[cfg(feature = "dht")]
pub use boson_dht as dht;

#[cfg(feature = "higgs")]
pub use boson_higgs as higgs;

#[cfg(feature = "director")]
pub use boson_director_client as director;

#[cfg(feature = "ion-store")]
pub use boson_ion_store_client as ion_store;

#[cfg(feature = "active-proxy")]
pub use boson_active_proxy_client as active_proxy;

#[cfg(feature = "messaging")]
pub use boson_messaging_client as messaging;

// The public API of the single rboson crate (`boson::Id`, `boson::dht::Node`, `boson::did`, ...),
// kept so applications written against it build unchanged.
pub use boson_api::{
    crypto_identity::{self, CryptoIdentity},
    cryptobox::{self, CryptoBox},
    errors::{self, Error, Result},
    id::{self, Id, DID_PREFIX},
    identity::{self, Identity},
    network::{self, Network},
    node_info::{self, NodeInfo},
    peer_info::{self, PeerBuilder, PeerInfo},
    signature::{self, Signature},
    value::{self, EncryptedBuilder, ImmutableBuilder, SignedBuilder, Value},
};
pub use boson_api::{as_ms, as_secs, elapsed_ms, locked, unwrap, unwrap_mut};

#[doc(hidden)]
pub use boson_api as core;

#[cfg(feature = "identifier")]
#[doc(hidden)]
pub use boson_identifier as did;

#[cfg(feature = "identifier")]
pub use boson_identifier::{
    cached_resolver, card, card_builder, credential, credential_builder, dht_registry,
    dht_resolver, didurl, filesystem_resolution_cache, proof, registry, resolution_cache,
    resolution_errors, resolver, verification_method, vouch, vouch_builder, w3c,
};

#[cfg(feature = "dht")]
pub use boson_dht::{
    connection_status::{self, ConnectionStatus},
    connection_status_listener::{self, ConnectionStatusListener},
    node::{self, Node},
};

#[cfg(feature = "active-proxy")]
#[doc(hidden)]
pub use boson_active_proxy_client as activeproxy;

#[cfg(feature = "active-proxy")]
pub use boson_active_proxy_client::{Client, Options};
