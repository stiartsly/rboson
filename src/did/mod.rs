
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
    v.as_ref().map(|s| s.is_empty()).unwrap_or(true)
}
pub(crate) fn is_none_or_zero<T: PartialEq + Default>(v: &Option<T>) -> bool {
    v.as_ref().map(|s| *s == T::default()).unwrap_or(true)
}

#[cfg(test)]
mod unitests {
    mod test_didurl;
    mod test_verification_method;
    mod test_proof;
}

// bytes serded as base64 URL safe without padding
mod serde_bytes_with_base64 {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use base64::{engine::general_purpose, Engine as _};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
    {
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(D::Error::custom)
    }
}
