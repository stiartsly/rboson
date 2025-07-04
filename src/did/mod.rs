
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

pub(crate) fn is_none_or_empty<T: IsEmpty>(v: &Option<T>) -> bool {
    v.as_ref().map(|s| s.is_empty()).unwrap_or(true)
}

pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for String {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for u64 {
    fn is_empty(&self) -> bool {
        *self == 0
    }
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
