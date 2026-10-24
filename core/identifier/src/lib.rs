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

// Migrated code that is not documented yet. Remove this allow as the crate is brought in line
// with the workspace lints.
#![allow(missing_docs)]

mod date;

pub mod did_constants;
pub mod didurl;
pub mod proof;
pub mod verification_method;

pub mod w3c {
    mod diddoc;
    mod diddoc_builder;
    mod format;
    mod vc;
    mod vc_builder;
    mod vp;
    mod vp_builder;

    pub use self::{
        diddoc::DIDDocument, diddoc_builder::DIDDocumentBuilder, format::W3CDIDFormat,
        vc::VerifiableCredential, vc_builder::VerifiableCredentialBuilder,
        vp::VerifiablePresentation, vp_builder::VerifiablePresentationBuilder,
    };
}

pub(crate) mod boson_identity_builder;
pub mod cached_resolver;
pub mod card;
pub mod card_builder;
pub mod credential;
pub mod credential_builder;
pub mod dht_registry;
pub mod dht_resolver;
pub mod filesystem_resolution_cache;
pub mod registry;
pub mod resolution_cache;
pub mod resolution_errors;
pub mod resolver;
pub mod vouch;
pub mod vouch_builder;

pub(crate) use crate::boson_identity_builder::BosonIdentityObjectBuilder;

pub use crate::{
    cached_resolver::CachedResolver,
    card::Card,
    card_builder::CardBuilder,
    credential::Credential,
    credential_builder::CredentialBuilder,
    dht_registry::DHTRegistry,
    dht_resolver::DHTResolver,
    did_constants::{self as constants, DID_METHOD, DID_SCHEME},
    didurl::DIDUrl,
    filesystem_resolution_cache::FileSystemResolutionCache,
    proof::Proof,
    registry::{PublishedCard, Registry, RegistryResolver},
    resolution_cache::ResolutionCache,
    resolution_errors::{RegistryError, ResolutionCacheError, ResolverError},
    resolver::{
        ResolutionMetadata, ResolutionOptions, ResolutionResult, ResolutionStatus, Resolver,
    },
    verification_method::{VerificationMethod, VerificationMethodType},
    vouch::Vouch,
    vouch_builder::VouchBuilder,
};
