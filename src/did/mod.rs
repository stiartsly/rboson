pub mod did_constants;
pub mod didurl;
pub mod proof;
pub mod verification_method;

pub mod w3c {
    mod diddoc;
    mod diddoc_builder;
    mod vc;
    mod vc_builder;
    mod vp;
    mod vp_builder;

    pub use self::{
        diddoc::DIDDocument, diddoc_builder::DIDDocumentBuilder, vc::VerifiableCredential,
        vc_builder::VerifiableCredentialBuilder, vp::VerifiablePresentation,
        vp_builder::VerifiablePresentationBuilder,
    };
}

pub(crate) mod boson_identity_object_builder;
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

pub(crate) use crate::did::boson_identity_object_builder::BosonIdentityObjectBuilder;

pub use crate::did::{
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
    registry::Registry,
    resolution_cache::ResolutionCache,
    resolution_errors::{RegistryError, ResolutionCacheError, ResolverError},
    resolver::{
        ResolutionMetadata, ResolutionOptions, ResolutionResult, ResolutionStatus, Resolver,
    },
    verification_method::{VerificationMethod, VerificationMethodType},
    vouch::Vouch,
    vouch_builder::VouchBuilder,
};

#[cfg(test)]
mod unitests {
    mod test_didurl;
    mod test_proof;
    mod test_resolution_cache;
    mod test_verification_method;
}
