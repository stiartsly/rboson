
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
