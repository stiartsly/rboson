use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, Duration};
use serde::{Deserialize, Serialize};

use crate::{
    as_secs,
    Id,
    signature,
    CryptoIdentity,
    core::{Error, Result},
};

use crate::did::{
    Credential,
    VouchBuilder,
    w3c::VerifiablePresentation as VP,
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Vouch {
    #[serde(rename = "id", skip_serializing_if = "crate::is_none_or_empty")]
    id: Option<String>,

    #[serde(rename = "t", skip_serializing_if = "crate::is_none_or_empty")]
    types: Option<Vec<String>>,

    #[serde(rename = "h")]
    holder: Id,

    #[serde(rename = "c", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<Credential>,

    #[serde(rename = "sat", skip_serializing_if = "crate::is_none_or_empty")]
    signed_at: Option<u64>,

    #[serde(rename = "sig", skip_serializing_if = "Vec::is_empty")]
    #[serde(with="super::serde_bytes_with_base64")]
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
        vp: Option<VP>
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

    pub(crate) fn signed(
        mut unsigned: Self,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>
    )-> Self {
        unsigned.signed_at = signed_at.map(|v|as_secs!(v));
        unsigned.signature = signature.unwrap_or_else(|| vec![]);
        unsigned
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn types(&self) -> Vec<&str> {
        self.types.as_ref().map(|t|
            t.iter().map(|v| v.as_str()).collect()
        ).unwrap_or_default()
    }

    pub fn holder(&self) -> &Id {
        &self.holder
    }

    pub fn credentials(&self) -> Vec<&Credential> {
        self.credentials.iter().collect()
    }

    pub fn credentials_by_type(&self, credential_type: &str) -> Vec<&Credential> {
        self.credentials.iter()
            .filter(|c| c.types().contains(&credential_type))
            .collect()
    }

    pub fn credentials_by_id(&self, id: &str) -> Vec<&Credential> {
        self.credentials.iter()
            .filter(|c| c.id() == id)
            .collect()
    }

    pub fn signed_at(&self) -> Option<SystemTime> {
        self.signed_at.map(|v|
            SystemTime::UNIX_EPOCH + Duration::from_secs(v)
        )
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
            &self.holder.to_signature_key()
        ).is_ok()
    }

    pub fn validate(&self) -> Result<()> {
        if self.signature.is_empty() {
            return Err(Error::Signature("Vouch signature is empty".into()));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(Error::Signature("Vouch signature is not valid".into())),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        if self.signature.is_empty() {
            self.into()
        } else {
            Vec::from(&Self::signed(self.clone(), None, None))
        }
    }

    pub fn vp(&self) -> Option<&VP> {
        self.vp.as_ref()
    }

    pub fn builder(holder: CryptoIdentity) -> VouchBuilder {
        VouchBuilder::new(holder)
    }
}

impl PartialEq<Vouch> for Vouch {
    fn eq(&self, other: &Vouch) -> bool {
        self.id == other.id &&
        self.types == other.types &&
        self.holder == other.holder &&
        self.credentials == other.credentials &&
        self.signed_at == other.signed_at &&
        self.signature == other.signature
    }
}

impl TryFrom<&str> for Vouch {
    type Error = Error;

    fn try_from(input: &str) -> Result<Self> {
        serde_json::from_str(input).map_err(|e| {
            Error::Argument(format!("Failed to parse Vouch from string: {}", e))
        })
    }
}

impl FromStr for Vouch {
    type Err = Error;

    fn from_str(data: &str) -> Result<Self> {
        Self::try_from(data)
    }
}

impl TryFrom<&[u8]> for Vouch {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|e| {
            Error::Argument(format!("Failed to parse Vouch from bytes: {}", e))
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
