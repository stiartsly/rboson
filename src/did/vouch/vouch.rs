use std::{fmt, str, time::SystemTime};
use serde::{Deserialize, Serialize};

use crate::did::date;
use crate::{
    errors::{ArgumentError, Error, Result, SignatureError},
    signature, utils::codec, CryptoIdentity, Id,
};

use crate::did::{
    w3c::VerifiablePresentation as VP,
    Credential,
    VouchBuilder
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Vouch {
    #[serde(rename = "id",
        default, skip_serializing_if = "codec::is_default")]
    id: Option<String>,

    #[serde(rename = "t",
        default, skip_serializing_if = "codec::is_default")]
    types: Option<Vec<String>>,

    #[serde(rename = "h")]
    holder: Id,

    #[serde(rename = "c", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<Credential>,

    #[serde(
        rename = "sat",
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "date::serialize_opt",
        deserialize_with = "date::deserialize_opt"
    )]
    signed_at: Option<SystemTime>,

    #[serde(
        rename = "sig",
        default,
        serialize_with = "codec::bytes::serialize",
        deserialize_with = "codec::bytes::deserialize",
        skip_serializing_if = "Vec::is_empty"
    )]
    signature: Vec<u8>,

    #[serde(skip)]
    vp: Option<VP>,
}

impl Vouch {
    pub(crate) fn unsigned(
        id: Option<String>,
        types: Option<Vec<String>>,
        holder: Id,
        credentials: Vec<Credential>,
        vp: Option<VP>,
    ) -> Self {
        Self {
            id,
            types,
            holder,
            credentials,
            signed_at: None,
            signature: vec![],
            vp,
        }
    }

    /// Stamps the signing time before the signature is computed.
    pub(crate) fn stamp_signed_at(mut self, signed_at: SystemTime) -> Self {
        self.signed_at = Some(signed_at);
        self
    }

    pub(crate) fn signed(
        mut unsigned: Self,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>,
    ) -> Self {
        if signed_at.is_some() {
            unsigned.signed_at = signed_at;
        }
        unsigned.signature = signature.unwrap_or_default();
        unsigned
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn types(&self) -> Vec<&str> {
        self.types
            .as_ref()
            .map(|t| t.iter().map(|v| v.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn holder(&self) -> &Id {
        &self.holder
    }

    pub fn credentials(&self) -> Vec<&Credential> {
        self.credentials.iter().collect()
    }

    pub fn credentials_by_type(&self, credential_type: &str) -> Vec<&Credential> {
        self.credentials
            .iter()
            .filter(|c| c.types().contains(&credential_type))
            .collect()
    }

    pub fn credential(&self, id: &str) -> Option<&Credential> {
        self.credentials.iter().find(|credential| credential.id() == id)
    }

    pub fn credentials_by_id(&self, id: &str) -> Vec<&Credential> {
        self.credentials.iter().filter(|c| c.id() == id).collect()
    }

    pub fn signed_at(&self) -> Option<SystemTime> {
        self.signed_at
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn is_genuine(&self) -> bool {
        if self.signature.len() != signature::Signature::BYTES {
            return false;
        }

        signature::verify(
            &self.to_sign_data(),
            &self.signature[..],
            &self.holder.to_signature_key(),
        )
        .is_ok()
            && self.credentials.iter().all(Credential::is_genuine)
    }

    pub fn validate(&self) -> Result<()> {
        if self.signature.is_empty() {
            return Err(ArgumentError::new("Vouch signature is empty"));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(SignatureError::new("Vouch signature is not valid")),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        serde_cbor::to_vec(&unsigned).unwrap_or_default()
    }

    pub fn vp(&self) -> Option<&VP> {
        self.vp.as_ref()
    }

    pub fn parse(json: &str) -> Result<Self> {
        Self::try_from(json)
    }

    pub fn parse_bytes(cbor: &[u8]) -> Result<Self> {
        Self::try_from(cbor)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode Vouch: {error}")).into()
        })
    }

    pub fn to_pretty_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode Vouch: {error}")).into()
        })
    }

    pub fn builder(holder: CryptoIdentity) -> VouchBuilder {
        VouchBuilder::new(holder)
    }
}

impl PartialEq<Vouch> for Vouch {
    fn eq(&self, other: &Vouch) -> bool {
        self.id == other.id
            && self.types == other.types
            && self.holder == other.holder
            && self.credentials == other.credentials
            && self.signed_at == other.signed_at
            && self.signature == other.signature
    }
}

impl TryFrom<&str> for Vouch {
    type Error = Error;

    fn try_from(input: &str) -> Result<Self> {
        serde_json::from_str(input).map_err(|e| {
            ArgumentError::new(format!("Failed to parse Vouch from string: {}", e)).into()
        })
    }
}

impl str::FromStr for Vouch {
    type Err = Error;

    fn from_str(data: &str) -> Result<Self> {
        Self::try_from(data)
    }
}

impl TryFrom<&[u8]> for Vouch {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|e| {
            ArgumentError::new(format!("Failed to parse Vouch from bytes: {}", e)).into()
        })
    }
}

impl From<&Vouch> for Vec<u8> {
    fn from(vouch: &Vouch) -> Self {
        serde_cbor::to_vec(vouch).unwrap()
    }
}

impl fmt::Display for Vouch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .fmt(f)
    }
}
