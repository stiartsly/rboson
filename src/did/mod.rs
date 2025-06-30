
pub(crate) mod did_constants;
pub mod didurl;
pub mod verification_method;
pub mod proof;

pub mod w3c {
    mod verifiable_credential;
    mod verifiable_credential_builder;
    mod verifiable_presentation;
    mod verifiable_presentation_builder;
    mod did_document;
    mod did_document_builder;

    pub use self::{
        verifiable_credential::VerifiableCredential,
        verifiable_credential_builder::VerifiableCredentialBuilder,
        verifiable_presentation::VerifiablePresentation,
        verifiable_presentation_builder::VerifiablePresentationBuilder,
        did_document::DIDDocument,
        did_document_builder::DIDDocumentBuilder
    };
}

pub(crate) mod boson_identity_object_builder;
pub mod credential;
pub mod credential_builder;
pub mod vouch;
pub mod vouch_builder;
pub mod card;
pub mod card_builder;

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
    credential::Credential,
    credential_builder::CredentialBuilder,
    vouch::Vouch,
    vouch_builder::VouchBuilder,

    did_constants::{
        DID_SCHEME,
        DID_METHOD
    }
};

pub(crate) fn is_none_or_empty(v: &Option<String>) -> bool {
    v.as_ref().map(|s| s.is_empty()).unwrap_or(false)
}
pub(crate) fn is_zero<T: PartialEq + Default>(v: &T) -> bool {
    *v == T::default()
}

#[cfg(test)]
mod unitests {
    mod test_didurl;
    mod test_verification_method;
}
