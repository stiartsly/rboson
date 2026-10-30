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
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
    time::SystemTime,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use boson_api::errors::{
    ArgumentError,
    BeforeValidPeriodError,
    ExpiredError,
    SignatureError,
    Result,
    Error,
};
use boson_api::{utils, CryptoIdentity, Id};
use crate::{
    date::date_opt,
    did_constants as constants,
    proof::{Proof, ProofPurpose, ProofType},
    w3c::VerifiableCredentialBuilder as VCBuilder,
    Credential,
    VerificationMethod,
    DIDUrl
};

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Eq)]
pub struct VerifiableCredential {
    #[serde(
        rename = "@context",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    contexts: Option<Vec<String>>,

    #[serde(rename = "id")]
    id: String,

    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    types: Option<Vec<String>>,

    #[serde(
        rename = "name",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    name: Option<String>,

    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    description: Option<String>,

    #[serde(
        rename = "issuer",
        with = "utils::id"
    )]
    issuer: Id,

    #[serde(
        rename = "validFrom",
        default,
        with = "date_opt",
    )]
    valid_from: Option<SystemTime>,

    #[serde(
        rename = "validUntil",
        default,
        with = "date_opt",
    )]
    valid_until: Option<SystemTime>,

    #[serde(rename = "credentialSubject")]
    subject: CredentialSubject,

    #[serde(
        rename = "proof",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    proof: Option<Proof>,
}

impl VerifiableCredential {
    pub(crate) fn unsigned(
        contexts: Vec<String>,
        id: String,
        types: Vec<String>,
        name: Option<String>,
        description: Option<String>,
        issuer: Id,
        valid_from: Option<SystemTime>,
        valid_until: Option<SystemTime>,
        subject: Option<Id>,
        claims: Map<String, Value>,
    ) -> Self {
        let contexts = match contexts.is_empty() {
            true => None,
            false => Some(contexts),
        };
        let types = match types.is_empty() {
            true => None,
            false => Some(types),
        };
        let subject = CredentialSubject::new(subject.unwrap_or(issuer.clone()), claims);
        Self {
            contexts,
            id,
            types,
            name,
            description,
            issuer,
            subject,
            valid_from,
            valid_until,
            proof: None,
        }
    }

    pub(crate) fn signed(mut vc: VerifiableCredential, proof: Proof) -> Self {
        vc.proof = Some(proof);
        vc
    }

    pub fn from_credential(credential: &Credential) -> Self {
        Self::from_cred(credential)
    }

    pub fn from_credential_with_type_contexts(
        credential: &Credential,
        type_contexts: Option<HashMap<&str, Vec<&str>>>,
    ) -> Self {
        Self::from_cred_with_type_contexts(credential, type_contexts)
    }

    pub fn from_cred(credential: &Credential) -> Self {
        Self::from_cred_with_type_contexts(credential, None)
    }

    pub(crate) fn from_cred_with_type_contexts(
        credential: &Credential,
        type_contexts: Option<HashMap<&str, Vec<&str>>>,
    ) -> Self {
        if let Some(vc) = credential.vc() {
            return vc.clone(); // If already a VC in credential, return it
        }

        let mut types: Vec<&str> = vec![constants::DEFAULT_VC_TYPE];
        let mut contexts: Vec<&str> = vec![
            constants::W3C_VC_CONTEXT,
            constants::BOSON_VC_CONTEXT,
            constants::W3C_ED25519_CONTEXT,
        ];

        for t in credential.types() {
            if t == constants::DEFAULT_VC_TYPE {
                continue; // Skip default type
            }
            types.push(t);

            let Some(extra_contexts) = type_contexts.as_ref() else {
                continue;
            };
            let Some(ctxs) = extra_contexts.get(t) else {
                continue;
            };
            for ctx in ctxs {
                if !contexts.contains(&ctx) {
                    contexts.push(ctx);
                }
            }
        }

        let did_url = DIDUrl::new(credential.subject().id(), None, None, Some(credential.id()));

        let subject = CredentialSubject::new(
            credential.subject().id().clone(),
            credential.subject().claims_map().clone(),
        );

        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            credential.signed_at().unwrap_or(SystemTime::now()),
            VerificationMethod::default_reference(credential.issuer()),
            ProofPurpose::AssertionMethod,
            credential.signature().to_vec(),
        );

        Self {
            contexts: Some(contexts.iter().map(|s| s.to_string()).collect()),
            id: did_url.to_string(),
            types: Some(types.iter().map(|s| s.to_string()).collect()),
            name: credential.name().map(|v| v.to_string()),
            description: credential.description().map(|v| v.to_string()),
            issuer: credential.issuer().clone(),
            valid_from: credential.valid_from(),
            valid_until: credential.valid_until(),
            subject,
            proof: Some(proof),
        }
    }

    pub fn contexts(&self) -> Vec<&str> {
        self.contexts
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn types(&self) -> Vec<&str> {
        self.types
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect())
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

    pub fn subject(&self) -> &CredentialSubject {
        &self.subject
    }

    pub fn proof(&self) -> &Proof {
        self.proof.as_ref().unwrap()
    }

    pub fn self_issued(&self) -> bool {
        self.issuer == self.subject.id
    }

    pub fn is_valid(&self) -> bool {
        if self.valid_from.is_none() && self.valid_until.is_none() {
            return true;
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
        self.proof
            .as_ref()
            .map(|v| v.verify(&self.issuer, &self.to_sign_data()))
            .unwrap_or(false)
    }

    pub fn validate(&self) -> Result<()> {
        let now = SystemTime::now();
        if self.valid_from.is_some_and(|value| now < value) {
            return Err(BeforeValidPeriodError::new("VC is not yet valid"));
        }
        if self.valid_until.is_some_and(|value| now > value) {
            return Err(ExpiredError::new("VC has expired"));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(SignatureError::new("VC signature is not valid")),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        let unsigned = self.to_unsigned_boson_credential();
        match self.proof.as_ref() {
            Some(proof) => unsigned.stamp_signed_at(proof.created()).to_sign_data(),
            None => unsigned.to_sign_data(),
        }
    }

    pub(crate) fn to_unsigned_boson_credential(&self) -> Credential {
        let id = self
            .id
            .parse::<DIDUrl>()
            .unwrap()
            .fragment()
            .unwrap()
            .to_string();
        let types = if let Some(ref t) = self.types {
            match t.is_empty() {
                true => None,
                false => Some(t.iter().map(|s| s.to_string()).collect()),
            }
        } else {
            None
        };

        Credential::unsigned(
            id,
            types,
            self.name.clone(),
            self.description.clone(),
            Some(self.issuer.clone()),
            self.valid_from,
            self.valid_until,
            self.subject.id.clone(),
            self.subject.claims.clone(),
            Some(self.clone()),
        )
    }

    pub fn to_credential(&self) -> Credential {
        self.to_boson_credential()
    }

    pub fn to_boson_credential(&self) -> Credential {
        Credential::signed(
            self.to_unsigned_boson_credential(),
            self.proof.as_ref().map(|proof| proof.created()),
            self.proof.as_ref().map(|proof| proof.proof_value().to_vec()),
        )
    }

    pub fn parse(json: &str) -> Result<Self> {
        Self::try_from(json)
    }

    pub fn parse_bytes(cbor: &[u8]) -> Result<Self> {
        Self::try_from(cbor)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode VerifiableCredential: {error}")).into()
        })
    }

    pub fn to_pretty_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode VerifiableCredential: {error}")).into()
        })
    }

    pub fn builder(issuer: CryptoIdentity) -> VCBuilder {
        VCBuilder::new(issuer)
    }
}

impl TryFrom<&str> for VerifiableCredential {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            ArgumentError::new(format!("Failed to parse VC from string: {}", e)).into()
        })
    }
}

impl FromStr for VerifiableCredential {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        serde_json::from_str(s).map_err(|e| {
            ArgumentError::new(format!("Failed to parse VC from string: {}", e)).into()
        })
    }
}

impl TryFrom<&[u8]> for VerifiableCredential {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data)
            .map_err(|e| ArgumentError::new(format!("Failed to parse VC from bytes: {}", e)).into())
    }
}

impl From<&VerifiableCredential> for Vec<u8> {
    fn from(vc: &VerifiableCredential) -> Self {
        serde_cbor::to_vec(&vc).unwrap()
    }
}

impl From<&VerifiableCredential> for Credential {
    fn from(vc: &VerifiableCredential) -> Self {
        vc.to_boson_credential()
    }
}

impl From<&Credential> for VerifiableCredential {
    fn from(credential: &Credential) -> Self {
        Self::from_cred(&credential)
    }
}

impl fmt::Display for VerifiableCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self).map_err(|_| fmt::Error)?.fmt(f)
    }
}

impl Hash for VerifiableCredential {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "boson.did.vc".hash(state);
        self.id.hash(state);
        if let Some(types) = &self.types {
            for t in types {
                t.hash(state);
            }
        }
        if let Some(name) = &self.name {
            name.hash(state);
        }
        if let Some(description) = &self.description {
            description.hash(state);
        }
        self.issuer.hash(state);
        self.valid_from.hash(state);
        self.valid_until.hash(state);
        self.proof.hash(state);
    }
}

impl PartialEq for VerifiableCredential {
    fn eq(&self, other: &Self) -> bool {
        self.contexts == other.contexts
            && self.id == other.id
            && self.types == other.types
            && self.name == other.name
            && self.description == other.description
            && self.issuer == other.issuer
            && self.valid_from == other.valid_from
            && self.valid_until == other.valid_until
            && self.subject == other.subject
            && self.proof == other.proof
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialSubject {
    #[serde(
        rename = "id",
        with = "utils::id",
    )]
    id: Id,

    claims: Map<String, Value>,
}

impl CredentialSubject {
    pub(crate) fn new(id: Id, claims: Map<String, Value>) -> Self {
        Self { id, claims }
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
        let mut map: HashMap<&str, T> = HashMap::new();
        for (k, v) in &self.claims {
            if let Ok(val) = serde_json::from_value(v.clone()) {
                map.insert(k.as_str(), val);
            }
        }
        map
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
