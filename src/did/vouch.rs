use std::fmt;
use std::time::{SystemTime, Duration};
use serde::{Deserialize, Serialize};

use crate::{
    as_secs,
    Id,
    error::{Error, Result},
    signature,
    CryptoIdentity,
};

use crate::did::{
    Credential,
    VouchBuilder,
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Vouch {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t", skip_serializing_if = "Vec::is_empty")]
    types: Vec<String>,

    #[serde(rename = "h")]
    holder: Id,

    #[serde(rename = "c", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<Credential>,

    #[serde(rename = "sat", skip_serializing_if = "super::is_zero")]
    signed_at: u64,

    #[serde(rename = "sig", skip_serializing_if = "Vec::is_empty")]
    signature: Vec<u8>,
}

#[allow(unused)]
impl Vouch {
    pub(crate) fn unsigned(
        id: String,
        types: Vec<String>,
        holder: Id,
        credentials: Vec<Credential>
    ) -> Self {
        Self {
            id,
            types,
            holder,
            credentials,
            signed_at: 0, // unsigned vouch has no signed_at
            signature: vec![],
        }
    }

    pub(crate) fn signed(
        mut unsigned: Self,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>
    )-> Self {
        unsigned.signed_at = signed_at.map(|v|as_secs!(v)).unwrap_or(0);
        unsigned.signature = signature.unwrap_or_else(|| vec![0u8; 0]);
        unsigned
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn types(&self) -> Vec<&str> {
        self.types.iter().map(|v| v.as_str()).collect()
    }

    pub fn holder(&self) -> &Id {
        &self.holder
    }

    pub fn credentials(&self) -> Vec<&Credential> {
        self.credentials.iter().collect()
    }

    pub fn signed_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.signed_at)
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn is_geniune(&self) -> bool {
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

        match self.is_geniune() {
            true => Ok(()),
            false => Err(Error::Signature("Vouch signature is not valid".into())),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        if self.signature.is_empty() {
            let card = Self::signed(self.clone(), None, None);
            Vec::from(&card)
        } else {
            Vec::from(self)
        }
    }

    pub fn builder(holder: &CryptoIdentity) -> VouchBuilder {
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

impl fmt::Display for Vouch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .fmt(f)
    }
}

impl TryFrom<&str> for Vouch {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            Error::Argument(format!("Failed to parse Vouch from string: {}", e))
        })
    }
}

impl TryFrom<&[u8]> for Vouch {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|e| {
            Error::Argument(format!("Failed to parse Vouch from bytes: {}", e))
        })
    }
}

impl From<&Vouch> for String {
    fn from(vouch: &Vouch) -> Self {
        serde_json::to_string(&vouch).unwrap()
    }
}

impl From<&Vouch> for Vec<u8> {
    fn from(vouch: &Vouch) -> Self {
        serde_json::to_vec(vouch).unwrap()
    }
}
