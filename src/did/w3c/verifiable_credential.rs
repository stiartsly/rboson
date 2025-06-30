use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    as_secs,
    Id,
    error::{Error, Result},
    CryptoIdentity,
};

use crate::did::{
    did_constants,
    proof::{Proof, ProofType, ProofPurpose},
    Credential,
    VerificationMethod,
    DIDUrl,
    w3c::VerifiableCredentialBuilder

};

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct VerifiableCredential {
    #[serde(rename = "@context", skip_serializing_if = "Vec::is_empty")]
    contexts: Vec<String>,

    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "type", skip_serializing_if = "Vec::is_empty")]
    types: Vec<String>,

    #[serde(rename = "name", skip_serializing_if = "crate::did::is_none_or_empty")]
    name: Option<String>,

    #[serde(rename = "description", skip_serializing_if = "crate::did::is_none_or_empty")]
    description: Option<String>,

    #[serde(rename = "issuer")]
    issuer: Id,

    #[serde(rename = "validFrom", skip_serializing_if = "crate::did::is_zero")]
    valid_from: u64,

    #[serde(rename = "validUntil", skip_serializing_if = "crate::did::is_zero")]
    valid_until: u64,

    #[serde(rename = "credentialSubject")]
    subject: CredentialSubject,

    #[serde(rename = "proof")]
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
        claims: Map<String, Value>
    ) -> Self {

        let subject = CredentialSubject::new(
            subject.unwrap_or(issuer.clone()),
            claims
        );
        Self {
            contexts,
            id,
            types,
            name,
            description,
            issuer,
            valid_from  : valid_from.map(|v| as_secs!(v)).unwrap_or(0),
            valid_until : valid_until.map(|v| as_secs!(v)).unwrap_or(0),
            subject,
            proof: None,
        }
    }

    pub(crate) fn signed(
        mut vc: VerifiableCredential,
        proof: Proof
    ) -> Self {
        vc.proof = Some(proof);
        vc
    }

    pub(crate) fn from_credential(credential: &Credential) -> Self {
        Self::from_credential_with_type_contexts(credential, HashMap::new())
    }

    pub(crate) fn from_credential_with_type_contexts(
        credential: &Credential,
        type_contexts: HashMap<String, Vec<String>>
    ) -> Self {

        if let Some(vc) = credential.vc() {
            return vc;
        }

        let mut types: Vec<String> = vec![
            did_constants::DEFAULT_VC_TYPE.into()
        ];
        let mut contexts: Vec<String> = vec![
            did_constants::W3C_VC_CONTEXT.into(),
            did_constants::BOSON_VC_CONTEXT.into(),
            did_constants::W3C_ED25519_CONTEXT.into()
        ];

        for type_ in &credential.types() {
            if type_ == &did_constants::DEFAULT_VC_TYPE {
                continue; // Skip default type
            }

            let t = type_.to_string();
            types.push(t.clone());
            let Some(type_contexts) = type_contexts.get(&t) else {
                continue; // Skip if no contexts for this type
            };

            for ctx in type_contexts {
                if !contexts.contains(ctx) {
                    contexts.push(ctx.clone());
                }
            }
        }

        let did_url = DIDUrl::new(
            credential.subject().id(),
            None,
            None,
            Some(credential.id()),
        );

        let subject = CredentialSubject::new(
            credential.subject().id().clone(),
            credential.subject().claims().clone()
        );

        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            credential.signed_at(),
            VerificationMethod::default_reference(credential.issuer()),
            ProofPurpose::AssertionMethod,
            credential.signature().to_vec()
        );

        Self {
            contexts,
            id          : did_url.to_string(),
            types,
            name        : credential.name().map(|v| v.to_string()),
            description : credential.description().map(|v| v.to_string()),
            issuer      : credential.issuer().clone(),
            valid_from  : as_secs!(credential.valid_from()),
            valid_until : as_secs!(credential.valid_until()),
            subject,
            proof       : Some(proof),
        }
    }

    pub fn contexts(&self) -> Vec<&str> {
        self.contexts.iter().map(|s| s.as_str()).collect()
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn types(&self) -> Vec<&str>{
        self.types.iter().map(|s| s.as_str()).collect()
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
        self.proof.as_ref().map(|v|v.verify(
            &self.issuer,
            &self.to_sign_data()
        )).unwrap_or(false)
    }

    pub fn validate(&self) -> Result<()> {
        let now = as_secs!(SystemTime::now());
        if self.valid_from != 0 && self.valid_from > now {
            return Err(Error::BeforeValidPeriod("Credential is not valid yet".into()));
        }
        if self.valid_until != 0 && self.valid_until < now {
            return Err(Error::Expired("Credential is expired".into()));
        }

        if !self.is_geniune() {
            return Err(Error::Signature("Credential proof is not valid".into()));
        }
        Ok(())
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        unimplemented!()
    }

    pub fn builder(issuer: CryptoIdentity) -> VerifiableCredentialBuilder {
        VerifiableCredentialBuilder::new(issuer)
    }
}

impl TryFrom<&str> for VerifiableCredential {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e|
            Error::Argument(format!("Failed to parse VC from string: {}", e))
        )
    }
}

impl TryFrom<&[u8]> for VerifiableCredential {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|e|
            Error::Argument(format!("Failed to parse VC from bytes: {}", e))
        )
    }
}

impl From<&VerifiableCredential> for String {
    fn from(vc: &VerifiableCredential) -> Self {
        serde_json::to_string(&vc).unwrap()
    }
}

impl From<&VerifiableCredential> for Vec<u8> {
    fn from(vc: &VerifiableCredential) -> Self {
        serde_json::to_vec(&vc).unwrap()
    }
}

impl From<&VerifiableCredential> for Credential {
    fn from(_vc: &VerifiableCredential) -> Self {
        unimplemented!()
    }
}

impl From<&Credential> for VerifiableCredential {
    fn from(credential: &Credential) -> Self {
        Self::from_credential(&credential)
    }
}

impl Hash for VerifiableCredential {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "boson.did.vc".hash(state);
        self.id.hash(state);
        for t in &self.types {
            t.hash(state);
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
        self.contexts == other.contexts &&
        self.id == other.id &&
        self.types == other.types &&
        self.name == other.name &&
        self.description == other.description &&
        self.issuer == other.issuer &&
        self.valid_from == other.valid_from &&
        self.valid_until == other.valid_until &&
        self.subject == other.subject &&
        self.proof == other.proof
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialSubject {
    #[serde(rename="id")]
    id: Id,

    claims: Map<String, Value>
}

impl CredentialSubject {
    pub(crate) fn new(id: Id, claims: Map<String, Value>) -> Self {
        Self { id, claims }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn claims(&self) -> &Map<String, Value> {
        &self.claims
    }

    pub fn claim(&self, key: &str) -> Option<&Value> {
        self.claims.get(key)
    }
}
