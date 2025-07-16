use std::fmt;
use std::str::FromStr;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

use crate::{
    Id,
    Error,
    CryptoIdentity,
    core::Result,
};

use crate::did::{
    did_constants as constants,
    DIDUrl,
    proof::{Proof, ProofType, ProofPurpose},
    VerificationMethod,
    Vouch,
    w3c::{
        VerifiableCredential as VC,
        VerifiablePresentationBuilder,
    }
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiablePresentation {
    #[serde(rename = "@context")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    contexts: Option<Vec<String>>,

    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    id: Option<String>,

    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    types: Option<Vec<String>>,

    #[serde(rename = "holder")]
    holder: Id,

    #[serde(rename = "verifiableCredential")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<VC>,

    #[serde(rename = "proof", skip_serializing_if = "Option::is_none")]
    proof: Option<Proof>
}

impl VerifiablePresentation {
    pub(crate) fn unsigned(
        contexts: Vec<String>,
        id: Option<String>,
        types: Vec<String>,
        holder: Id,
        credentials: Vec<VC>,
    ) -> Self {

        let contexts = match contexts.is_empty() {
            true => None,
            false => Some(contexts),
        };
        let types = match types.is_empty() {
            true => None,
            false => Some(types),
        };
        Self {
            contexts,
            id,
            types,
            holder,
            credentials,
            proof: None,
        }
    }

    pub(crate) fn signed(mut unsigned: Self, proof: Option<Proof>) -> Self {
        unsigned .proof = proof;
        unsigned
    }

    pub fn from_vouch(vouch: &Vouch) -> Self {
        Self::from_vouch_with_type_contexts(
            vouch,
            HashMap::new()
        )
    }

    pub fn from_vouch_with_type_contexts(
        vouch: &Vouch,
        type_contexts: HashMap<&str, Vec<String>>
    ) -> Self{
        if let Some(vp) = vouch.vp() {
            return vp.clone();
        }

        let mut types: Vec<&str> = vec![
            constants::DEFAULT_VP_TYPE
        ];
        let mut contexts: Vec<&str> = vec![
            constants::W3C_VC_CONTEXT,
            constants::BOSON_VC_CONTEXT,
            constants::W3C_ED25519_CONTEXT
        ];

        for t in vouch.types() {
            if t == constants::DEFAULT_VP_TYPE {
                continue;   // skip default VP type
            }
            types.push(t);

            let Some(ctxs) = type_contexts.get(t) else {
                continue;
            };

            for ctxt in ctxs {
                if !contexts.contains(&ctxt.as_str()) {
                    contexts.push(ctxt.as_str());
                };
            };
        }

        let id = if let Some(id) = vouch.id() {
            let did_url = DIDUrl::new(vouch.holder(), None, None, Some(id));
            Some(did_url.to_string())
        } else {
            None
        };

        let credentials = vouch.credentials().iter()
            .map(|c| VC::from(*c))
            .collect::<Vec<_>>();

        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            vouch.signed_at().unwrap(),
            VerificationMethod::default_reference(vouch.holder()),
            ProofPurpose::AssertionMethod,
            vouch.signature().to_vec(),
        );

        Self {
            contexts    : Some(contexts.iter().map(|s| s.to_string()).collect()),
            id,
            types       : Some(types.iter().map(|t| t.to_string()).collect()),
            holder      : vouch.holder().clone(),
            credentials,
            proof       : Some(proof)
        }
    }

    pub fn contexts(&self) -> Vec<&str> {
        self.contexts.as_ref().map(|v|
            v.iter().map(|c| c.as_str()).collect()
        ).unwrap_or_default()
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn types(&self) -> Vec<&str> {
        self.types.as_ref().map(|v|
            v.iter().map(|t| t.as_str()).collect()
        ).unwrap_or_default()
    }

    pub fn holder(&self) -> &Id {
        &self.holder
    }

    pub fn credentials(&self) -> Vec<&VC> {
        self.credentials.iter().collect()
    }

    pub fn credentials_by_type(&self, credential_type: &str) -> Vec<&VC> {
        self.credentials.iter().filter(|vc|
            vc.types().contains(&credential_type)
        ).collect()
    }

    pub fn credential(&self, id: &str) -> Option<&VC> {
        let did_url = if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            id.parse::<DIDUrl>().ok()?
        } else {
            DIDUrl::new(self.holder(), None, None, Some(id))
        };
        self.credential_by_didurl(&did_url)
    }

    pub fn credential_by_didurl(&self, id: &DIDUrl) -> Option<&VC> {
        self.credentials.iter().find(|&vc| {
            vc.id() == &id.to_string()
        })
    }

    pub fn proof(&self) -> Option<&Proof> {
        self.proof.as_ref()
    }

    pub fn validate(&self) -> Result<()> {
         match self.is_genuine() {
            true => Ok(()),
            false => Err(Error::Signature("VP signature is not valid".to_string())),
        }
    }

    pub fn is_genuine(&self) -> bool {
        self.proof.as_ref().map(|v|v.verify(
            self.holder(),
            &self.to_sign_data()
        )).unwrap_or(false)
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        self.to_unsigned_boson_vouch().to_sign_data()
    }

    pub fn to_unsigned_boson_vouch(&self) -> Vouch {
        let id = if let Some(id) = self.id.as_ref() {
            Some(id.parse::<DIDUrl>().unwrap().fragment().unwrap().to_string()) // TODO:
        } else {
            None
        };
        let types = if let Some(ref t) = self.types {
            match t.is_empty() {
                true => None,
                false => Some(t.iter().filter(|t| t.as_str() != constants::DEFAULT_VP_TYPE).cloned().collect()),
            }
        } else {
            None
        };

        Vouch::unsigned(
            id,
            types,
            self.holder.clone(),
            self.credentials.iter().map(|c| c.to_boson_credential()).collect(),
            Some(self.clone()),
        )
    }

    pub fn to_boson_vouch(&self) -> Vouch {

        Vouch::signed(
            self.to_unsigned_boson_vouch(),
            self.proof.as_ref().map(|p| p.created()),
            self.proof.as_ref().map(|p| p.proof_value().to_vec())
        )
    }

    pub fn builder(holder: CryptoIdentity) -> VerifiablePresentationBuilder {
        VerifiablePresentationBuilder::new(holder)
    }
}

impl TryFrom<&str> for VerifiablePresentation {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e|
            Error::Argument(format!("Failed to parse VP from string: {}", e))
        )
    }
}

impl FromStr for VerifiablePresentation {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

impl TryFrom<&[u8]> for VerifiablePresentation {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|e|
            Error::Argument(format!("Failed to parse VP from bytes: {}", e))
        )
    }
}

impl From<&VerifiablePresentation> for Vec<u8> {
    fn from(vp: &VerifiablePresentation) -> Self {
        serde_cbor::to_vec(&vp).unwrap()
    }
}

impl From<&VerifiablePresentation> for Vouch {
    fn from(vp: &VerifiablePresentation) -> Self {
        vp.to_boson_vouch()
    }
}

impl From<&Vouch> for VerifiablePresentation {
    fn from(vouch: &Vouch) -> Self {
        Self::from_vouch(&vouch)
    }
}

impl fmt::Display for VerifiablePresentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

impl Hash for VerifiablePresentation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.contexts.hash(state);
        self.types.hash(state);
        for credential in self.credentials.iter() {
            credential.hash(state);
        }
    }
}
