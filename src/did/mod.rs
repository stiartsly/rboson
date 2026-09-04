
pub mod did_constants;
pub mod didurl;
pub mod verification_method;
pub mod proof;

pub mod w3c {
    mod vc;
    mod vc_builder;
    mod vp;
    mod vp_builder;
    mod diddoc;
    mod diddoc_builder;

    pub use self::{
        vc::VerifiableCredential,
        vc_builder::VerifiableCredentialBuilder,
        vp::VerifiablePresentation,
        vp_builder::VerifiablePresentationBuilder,
        diddoc::DIDDocument,
        diddoc_builder::DIDDocumentBuilder
    };
}

pub(crate) mod boson_identity_object_builder;
pub mod credential;
pub mod credential_builder;
pub mod vouch;
pub mod vouch_builder;
pub mod card;
pub mod card_builder;
pub mod resolver;
pub mod registry;
pub mod dht_resolver;
pub mod dht_registry;
pub mod cached_resolver;
pub mod resolution_cache;
pub mod filesystem_resolution_cache;
pub mod resolution_errors;

pub(crate) use crate::did::{
    boson_identity_object_builder::BosonIdentityObjectBuilder,
};

pub use crate::did::{
    didurl::DIDUrl,
    proof::Proof,
    verification_method::{
        VerificationMethod,
        VerificationMethodType
    },

    card::Card,
    card_builder::CardBuilder,
    resolver::{Resolver, ResolutionStatus, ResolutionOptions, ResolutionMetadata, ResolutionResult},
    registry::Registry,
    dht_resolver::DHTResolver,
    dht_registry::DHTRegistry,
    cached_resolver::CachedResolver,
    resolution_cache::ResolutionCache,
    filesystem_resolution_cache::FileSystemResolutionCache,
    resolution_errors::{RegistryError, ResolverError, ResolutionCacheError},
    credential::Credential,
    credential_builder::CredentialBuilder,
    vouch::Vouch,
    vouch_builder::VouchBuilder,

    did_constants::{
        self as constants,
        DID_SCHEME,
        DID_METHOD,
    }
};

#[cfg(test)]
mod unitests {
    mod test_didurl;
    mod test_verification_method;
    mod test_proof;
}
