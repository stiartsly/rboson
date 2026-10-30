// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    hash::Hash,
    str::FromStr,
    time::SystemTime,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use boson_api::errors::{
    ArgumentError,
    BeforeValidPeriodError,
    ExpiredError,
    SignatureError,
    Result};
use boson_api::{signature, utils::{self, codec}, CryptoIdentity, Id};
use crate::{
    date::date_opt,
    w3c::VerifiableCredential as VC,
    CredentialBuilder
};

#[derive(Debug, Clone, Eq, Hash, Serialize)]
pub struct Credential {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t",
        default, skip_serializing_if = "utils::is_default")]
    types: Option<Vec<String>>,

    #[serde(rename = "n",
        default, skip_serializing_if = "utils::is_default")]
    name: Option<String>,

    #[serde(rename = "d",
        default, skip_serializing_if = "utils::is_default")]
    description: Option<String>,

    #[serde(rename = "i")]
    issuer: Id,

    #[serde(
        rename = "v",
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "date_opt::serialize",
        deserialize_with = "date_opt::deserialize"
    )]
    valid_from: Option<SystemTime>,

    #[serde(
        rename = "e",
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "date_opt::serialize",
        deserialize_with = "date_opt::deserialize"
    )]
    valid_until: Option<SystemTime>,

    #[serde(rename = "s")]
    subject: Subject,

    #[serde(
        rename = "sat",
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "date_opt::serialize",
        deserialize_with = "date_opt::deserialize"
    )]
    signed_at: Option<SystemTime>,

    #[serde(
        rename = "sig",
        serialize_with = "codec::bytes::serialize",
        deserialize_with = "codec::bytes::deserialize",
        skip_serializing_if = "Vec::is_empty"
    )]
    signature: Vec<u8>,

    #[serde(skip)]
    vc: Option<VC>,
}

impl Credential {
    pub(crate) fn unsigned(
        id: String,
        types: Option<Vec<String>>,
        name: Option<String>,
        description: Option<String>,
        issuer: Option<Id>,
        valid_from: Option<SystemTime>,
        valid_until: Option<SystemTime>,
        subject: Id,
        claims: Map<String, Value>,
        vc: Option<VC>,
    ) -> Self {
        Self {
            id,
            types,
            name,
            description,
            issuer: issuer.clone().unwrap_or_else(|| subject.clone()),
            valid_from,
            valid_until,
            subject: Subject::new(subject, claims).implicit_for_issuer(
                issuer.as_ref().unwrap_or(&subject),
            ),
            signed_at: None,
            signature: vec![],
            vc,
        }
    }

    /// Stamps the signing time before the signature is computed.
    pub(crate) fn stamp_signed_at(mut self, signed_at: SystemTime) -> Self {
        self.signed_at = Some(signed_at);
        self
    }

    pub(crate) fn signed(
        mut unsigned: Credential,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>,
    ) -> Self {
        if signed_at.is_some() {
            unsigned.signed_at = signed_at;
        }
        unsigned.signature = signature.unwrap_or_default();
        unsigned
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn types(&self) -> Vec<&str> {
        self.types
            .as_ref()
            .map(|t| t.iter().map(|v| v.as_str()).collect())
            .unwrap_or_default()
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
        self.valid_from
    }

    pub fn valid_until(&self) -> Option<SystemTime> {
        self.valid_until
    }

    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    pub fn signed_at(&self) -> Option<SystemTime> {
        self.signed_at
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

        let now = SystemTime::now();
        if self.valid_from.is_some_and(|value| now < value) {
            return false;
        }
        if self.valid_until.is_some_and(|value| now > value) {
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
        )
        .is_ok()
    }

    pub fn validate(&self) -> Result<()> {
        let now = SystemTime::now();
        if self.valid_from.is_some_and(|value| now < value) {
            return Err(BeforeValidPeriodError::new("Credential is not yet valid"));
        }
        if self.valid_until.is_some_and(|value| now > value) {
            return Err(ExpiredError::new("Credential has expired"));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(SignatureError::new("Credential signature is not valid")),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        serde_cbor::to_vec(&unsigned).unwrap_or_default()
    }

    pub fn vc(&self) -> Option<&VC> {
        self.vc.as_ref()
    }

    pub fn parse(json: &str) -> Result<Self> {
        Self::try_from(json)
    }

    pub fn parse_bytes(cbor: &[u8]) -> Result<Self> {
        Self::try_from(cbor)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode Credential: {error}")).into()
        })
    }

    pub fn to_pretty_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode Credential: {error}")).into()
        })
    }

    pub fn builder(issuer: CryptoIdentity) -> CredentialBuilder {
        CredentialBuilder::new(issuer)
    }
}

impl PartialEq<Self> for Credential {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.types == other.types
            && self.name == other.name
            && self.description == other.description
            && self.issuer == other.issuer
            && self.valid_from == other.valid_from
            && self.valid_until == other.valid_until
            && self.subject == other.subject
            && self.signed_at == other.signed_at
            && self.signature == other.signature
    }
}

impl fmt::Display for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self).map_err(|_| fmt::Error)?.fmt(f)
    }
}

#[derive(Deserialize)]
struct CredentialData {
    #[serde(rename = "id")]
    id: String,
    #[serde(
        rename = "t",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    types: Option<Vec<String>>,

    #[serde(
        rename = "n",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    name: Option<String>,

    #[serde(
        rename = "d",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    description: Option<String>,

    #[serde(rename = "i")]
    issuer: Id,

    #[serde(rename = "v", default, with = "date_opt")]
    valid_from: Option<SystemTime>,

    #[serde(rename = "e", default, with = "date_opt")]
    valid_until: Option<SystemTime>,

    #[serde(rename = "s")]
    subject: SubjectDataOwned,

    #[serde(rename = "sat", default, with = "date_opt")]
    signed_at: Option<SystemTime>,

    #[serde(rename = "sig", default, with = "utils::bytes")]
    signature: Vec<u8>,
}

impl<'de> Deserialize<'de> for Credential {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = CredentialData::deserialize(deserializer)?;
        let subject_id = data.subject.id.unwrap_or(data.issuer);
        let implicit = subject_id == data.issuer;
        Ok(Self {
            id: data.id,
            types: data.types.filter(|types| !types.is_empty()),
            name: data.name,
            description: data.description,
            issuer: data.issuer,
            valid_from: data.valid_from,
            valid_until: data.valid_until,
            subject: Subject {
                id: subject_id,
                claims: data.subject.claims,
                implicit,
            },
            signed_at: data.signed_at,
            signature: data.signature,
            vc: None,
        })
    }
}

impl TryFrom<&str> for Credential {
    type Error = Box<dyn Error>;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            ArgumentError::new(format!("Failed to parse Credential from string: {}", e)).into()
        })
    }
}

impl TryFrom<&[u8]> for Credential {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|e| {
            ArgumentError::new(format!("Failed to parse Credential from CBOR: {}", e)).into()
        })
    }
}

impl FromStr for Credential {
    type Err = Box<dyn std::error::Error>;

    fn from_str(data: &str) -> Result<Self> {
        Self::try_from(data)
    }
}

impl From<&Credential> for Vec<u8> {
    fn from(cred: &Credential) -> Self {
        serde_cbor::to_vec(cred).unwrap()
    }
}

impl From<&Credential> for String {
    fn from(cred: &Credential) -> Self {
        serde_json::to_string(cred).unwrap()
    }
}

#[derive(Debug, Clone, Default, Eq)]
pub struct Subject {
    id: Id,
    claims: Map<String, Value>,
    implicit: bool,
}

impl Subject {
    pub fn new(id: Id, claims: Map<String, Value>) -> Self {
        Subject {
            id,
            claims,
            implicit: false,
        }
    }

    fn implicit_for_issuer(mut self, issuer: &Id) -> Self {
        self.implicit = &self.id == issuer;
        self
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn claims_map(&self) -> &Map<String, Value> {
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
        self.claims.iter().find_map(|(k, v)| match k == key {
            true => serde_json::from_value(v.clone()).ok(),
            false => None,
        })
    }
}

impl Serialize for Subject {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SubjectData {
            id: if self.implicit { None } else { Some(self.id) },
            claims: &self.claims,
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct SubjectData<'a> {
    #[serde(rename = "id", default, skip_serializing_if = "Option::is_none")]
    id: Option<Id>,
    #[serde(flatten)]
    claims: &'a Map<String, Value>,
}

#[derive(Deserialize)]
struct SubjectDataOwned {
    #[serde(rename = "id", default)]
    id: Option<Id>,
    #[serde(flatten)]
    claims: Map<String, Value>,
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
        self.id == other.id && self.claims == other.claims
    }
}
