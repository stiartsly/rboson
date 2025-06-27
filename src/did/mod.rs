pub(crate) mod card;
pub(crate) mod card_builder;

pub(crate) mod credential;
pub(crate) mod credential_builder;

pub(crate) mod boson_identity_object_builder;
pub(crate) mod did_constants;
pub(crate) mod did_document;
pub(crate) mod did_document_builder;
pub(crate) mod did_url;
pub(crate) mod verification_method;
pub(crate) mod proof;
pub(crate) mod vouch;
pub(crate) mod vouch_builder;
pub(crate) mod verifiable_credential;
pub(crate) mod verifiable_credential_builder;
pub(crate) mod verifiable_presentation;
pub(crate) mod verifiable_presentation_builder;

pub use crate::did::{
    did_url::DIDUrl,
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

    verifiable_credential::VerifiableCredential,
    verifiable_credential_builder::VerifiableCredentialBuilder,
    verifiable_presentation::VerifiablePresentation,
    verifiable_presentation_builder::VerifiablePresentationBuilder,
    did_document::DIDDocument,
    did_document_builder::DIDDocumentBuilder,

    did_constants::{
        DID_SCHEME,
        DID_METHOD
    }
};

pub(crate) use self::{
    boson_identity_object_builder::BosonIdentityObjectBuilder
};

pub(crate) fn is_none_or_empty(v: &Option<String>) -> bool {
    v.as_ref().map(|s| s.is_empty()).unwrap_or(false)
}

pub(crate) fn is_zero<T: PartialEq + Default>(v: &T) -> bool {
    *v == T::default()
}
