use std::fmt;
use std::collections::HashMap;
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
    CryptoIdentity,
};

use crate::did::{
    CredentialBuilder,
    w3c::VerifiableCredential as VC,
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Credential {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t", skip_serializing_if = "super::is_none_or_empty")]
    types: Option<Vec<String>>,

    #[serde(rename = "n", skip_serializing_if = "super::is_none_or_empty")]
    name: Option<String>,

    #[serde(rename = "d", skip_serializing_if = "super::is_none_or_empty")]
    description: Option<String>,

    #[serde(rename = "i")]
    issuer: Id,

    #[serde(rename = "v", skip_serializing_if = "super::is_none_or_empty")]
    valid_from: Option<u64>,

    #[serde(rename = "e", skip_serializing_if = "super::is_none_or_empty")]
    valid_until: Option<u64>,

    #[serde(rename = "s")]
    subject: Subject,

    #[serde(rename = "sat", skip_serializing_if = "super::is_none_or_empty")]
    signed_at: Option<u64>,

    #[serde(rename = "sig", skip_serializing_if = "Vec::is_empty")]
    #[serde(with="super::serde_bytes_with_base64")]
    signature: Vec<u8>,

    #[serde(skip_serializing, skip_deserializing)]
    vc: Option<VC>,
}

impl Credential {
    pub(crate) fn unsigned(
        id: String,
        types: Option<Vec<String>>,
        name: Option<String>,
        description: Option<String>,
        issuer: Option<Id>,
        valid_from: Option<u64>,
        valid_until: Option<u64>,
        subject: Id,
        claims: Map<String, Value>,
        vc: Option<VC>,
    ) -> Self {
        Self {
            id,
            types,
            name,
            description,
            issuer      : issuer.unwrap_or_else(|| subject.clone()),
            valid_from,
            valid_until,
            subject     : Subject::new(subject, claims),
            signed_at   : None,
            signature   : vec![],
            vc,
        }
    }

    pub(crate) fn signed(
        mut unsigned: Credential,
        signed_at: Option<u64>,
        signature: Option<Vec<u8>>
    ) -> Self {
        unsigned.signed_at = signed_at;
        unsigned.signature = signature.unwrap_or_else(|| vec![0u8; 0]);
        unsigned
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn types(&self) -> Vec<&str> {
        self.types.as_ref().map(|t|
            t.iter().map(|v| v.as_str()).collect()
        ).unwrap_or_default()
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

    pub fn valid_from(&self) -> Option<SystemTime> {
        self.valid_from.map(|v|
            SystemTime::UNIX_EPOCH + Duration::from_secs(v)
        )
    }

    pub fn valid_until(&self) -> Option<SystemTime> {
        self.valid_until.map(|v|
            SystemTime::UNIX_EPOCH + Duration::from_secs(v)
        )
    }

    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    pub fn signed_at(&self) -> Option<SystemTime> {
        self.signed_at.map(|v|
            SystemTime::UNIX_EPOCH + Duration::from_secs(v)
        )
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn self_issued(&self) -> bool {
        &self.issuer == self.subject.id()
    }

    pub fn is_valid(&self) -> bool {
        if self.valid_from.is_none() && self.valid_until.is_none() {
            return true; // No validity constraints
        }

        let now = as_secs!(SystemTime::now());
        if self.valid_from.map(|v| v > now).unwrap_or(false) {
            return false;
        }
        if self.valid_until.map(|v| v < now).unwrap_or(false) {
            return false;
        }
        true
    }

    pub fn is_genuine(&self) -> bool {
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
        let now = as_secs!(SystemTime::now());
        if self.valid_from.is_some() && self.valid_from.unwrap() > now {
            return Err(Error::BeforeValidPeriod("Credential is not yet valid".into()));
        }
        if self.valid_until.is_some() && self.valid_until.unwrap() < now {
            return Err(Error::Expired("Credential has expired".into()));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(Error::Signature("Credential signature is not valid".into())),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        match self.signature.is_empty() {
            true    => Vec::from(self),
            false   => Vec::from(&Self::signed(self.clone(), None, None))
        }
    }

    pub fn vc(&self) -> Option<&VC> {
        self.vc.as_ref()
    }

    pub fn builder(issuer: CryptoIdentity) -> CredentialBuilder {
        CredentialBuilder::new(issuer)
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

    pub(crate) fn claims_map(&self) -> &Map<String, Value> {
        &self.claims
    }

    pub fn claims<'a, T>(&'a self) -> HashMap<&'a str, T>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut claims_map: HashMap<&str, T> = HashMap::new();
        for (k, v) in &self.claims {
            if let Ok(val) = serde_json::from_value(v.clone()) {
                claims_map.insert(k.as_str(), val);
            }
        }
        claims_map
    }

    pub fn claim<T>(&self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.claims.iter().find_map(|(k, v)| {
            match k == key {
                true => serde_json::from_value(v.clone()).ok(),
                false => None,
            }
        })
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
