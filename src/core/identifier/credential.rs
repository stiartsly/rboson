use std::fmt;
use std::time::{Duration,SystemTime};
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    as_secs,
    Id,
    Error,
    error::Result,
    signature,
    core::crypto_identity::CryptoIdentity,
};

use crate::core::identifier::{
    CredentialBuilder,
    VerifiableCredential,
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Credential {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t", skip_serializing_if = "Vec::is_empty")]
    types: Vec<String>,

    #[serde(rename = "n", skip_serializing_if = "super::is_none_or_empty")]
    name: Option<String>,

    #[serde(rename = "d", skip_serializing_if = "super::is_none_or_empty")]
    description: Option<String>,

    #[serde(rename = "i")]
    issuer: Id,

    #[serde(rename = "v", skip_serializing_if = "super::is_zero")]
    valid_from: u64,

    #[serde(rename = "e", skip_serializing_if = "super::is_zero")]
    valid_until: u64,

    #[serde(rename = "s")]
    subject: Subject,

    #[serde(rename = "sat", skip_serializing_if = "super::is_zero")]
    signed_at: u64,

    #[serde(rename = "sig")]
    signature: Vec<u8>
}

impl Credential {
    pub(crate) fn unsigned(
        id: String,
        types: Vec<String>,
        name: Option<String>,
        description: Option<String>,
        issuer: Option<Id>,
        valid_from: Option<SystemTime>,
        valid_until: Option<SystemTime>,
        subject: Id,
        claims: Map<String, Value>
    ) -> Self {
        Self {
            id,
            types,
            name,
            description,
            issuer      : issuer.unwrap_or_else(|| subject.clone()),
            valid_from  : valid_from.map_or(0, |t| as_secs!(t)),
            valid_until : valid_until.map_or(0, |t| as_secs!(t)),
            subject     : Subject::new(subject, claims),
            signed_at   : 0,
            signature   : vec![]
        }
    }

    pub(crate) fn signed(
        mut unsigned: Credential,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>
    ) -> Self {
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

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn issuer(&self) -> &Id {
        &self.issuer
    }

    pub fn valid_from(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.valid_from)
    }

    pub fn valid_until(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.valid_until)
    }

    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    pub fn signed_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.signed_at)
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn self_issued(&self) -> bool {
        &self.issuer == self.subject.id()
    }

    pub fn is_valid(&self) -> bool {
        if self.valid_from == 0 && self.valid_until == 0 {
            return true; // No validity constraints
        }

        let now = as_secs!(SystemTime::now());
        if self.valid_from > now {
            return false;
        }
        if self.valid_until < now && self.valid_until > 0 {
            return false;
        }
        true
    }

    pub fn is_geniune(&self) -> bool {
        if self.signature.len() != signature::Signature::BYTES {
            return false;
        }

        signature::verify(
            &self.to_sign_data(),
            &self.signature,
            &self.issuer().to_signature_key(),
        ).is_ok()
    }

    pub fn validate(&self) -> Result<()> {
        match self.is_geniune() {
            true => Ok(()),
            false => Err(Error::Signature("Credential signature is not valid".into())),
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

    pub(crate) fn vc(&self) -> Option<VerifiableCredential> {
        unimplemented!()
    }
}

impl PartialEq<Self> for Credential {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id &&
        self.types == other.types &&
        self.name == other.name &&
        self.description == other.description &&
        self.issuer == other.issuer &&
        self.valid_from == other.valid_from &&
        self.valid_until == other.valid_until &&
        self.subject == other.subject &&
        self.signature == other.signature
    }
}

impl fmt::Display for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

impl TryFrom<&str> for Credential {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e|
            Error::Argument(format!("Failed to parse Credential from string: {}", e))
        )
    }
}

impl TryFrom<&[u8]> for Credential {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|e|
            Error::Argument(format!("Failed to parse Credential from bytes: {}", e))
        )
    }
}

impl From<&Credential> for String {
    fn from(cred: &Credential) -> Self {
        serde_json::to_string(&cred).unwrap()
    }
}

impl From<&Credential> for Vec<u8> {
    fn from(cred: &Credential) -> Self {
        serde_json::to_vec(cred).unwrap()
    }
}

impl Credential {
    pub fn builder(issuer: CryptoIdentity) -> CredentialBuilder {
        CredentialBuilder::new(issuer)
    }
}

#[derive(Debug, Clone, Default, Eq, Serialize, Deserialize)]
pub struct Subject {
    #[serde(rename = "id")]
    id: Id,

    claims: Map<String, Value>,
}

impl Subject {
    pub fn new(id: Id, claims: Map<String, Value>) -> Self {
        Subject { id, claims }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn claims(&self) -> &Map<String, Value> {
        &self.claims
    }

    pub fn claim(&self, key: &str) -> Option<&Value> {
        self.claims.iter().find_map(|(k, v)|
            match k == key {
                true => Some(v),
                false => None,
            }
        )
    }
}

impl std::hash::Hash for Subject {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        for (key, value) in &self.claims {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl PartialEq<Self> for Subject {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id &&
        self.claims == other.claims
    }
}
