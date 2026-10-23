use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::SystemTime;

use crate::date;
use boson_api::{signature, Id, utils::codec};

use crate::{DIDUrl, VerificationMethod};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProofType {
    Ed25519Signature2020,
}

impl fmt::Display for ProofType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ProofType::Ed25519Signature2020 => "Ed25519Signature2020",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProofPurpose {
    #[serde(rename = "assertionMethod")]
    AssertionMethod,
    #[serde(rename = "authentication")]
    Authentication,
    #[serde(rename = "capabilityInvocation")]
    CapabilityInvocation,
    #[serde(rename = "capabilityDelegation")]
    CapabilityDelegation,
}

impl fmt::Display for ProofPurpose {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ProofPurpose::AssertionMethod => "assertionMethod",
            ProofPurpose::Authentication => "authentication",
            ProofPurpose::CapabilityInvocation => "capabilityInvocation",
            ProofPurpose::CapabilityDelegation => "capabilityDelegation",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Proof {
    #[serde(rename = "type")]
    proof_type: ProofType,

    #[serde(rename = "created", with = "date")]
    created: SystemTime,

    #[serde(rename = "verificationMethod")]
    verification_method: VerificationMethod,

    #[serde(rename = "proofPurpose")]
    proof_purpose: ProofPurpose,

    #[serde(
        rename = "proofValue",
        serialize_with = "codec::bytes::serialize",
        deserialize_with = "codec::bytes::deserialize"
    )]
    proof_value: Vec<u8>,
}

impl Proof {
    pub(crate) fn new(
        proof_type: ProofType,
        created: SystemTime,
        verification_method: VerificationMethod,
        proof_purpose: ProofPurpose,
        proof_value: Vec<u8>,
    ) -> Self {
        Self {
            proof_type,
            created,
            verification_method,
            proof_purpose,
            proof_value,
        }
    }

    pub fn types(&self) -> ProofType {
        self.proof_type
    }

    pub fn created(&self) -> SystemTime {
        self.created
    }

    pub fn verification_method(&self) -> &VerificationMethod {
        &self.verification_method
    }

    pub fn purpose(&self) -> ProofPurpose {
        self.proof_purpose
    }

    pub fn proof_value(&self) -> &[u8] {
        &self.proof_value
    }

    pub(crate) fn verify(&self, subject: &Id, data: &[u8]) -> bool {
        if self.proof_value.len() != signature::Signature::BYTES {
            return false;
        }

        match self.proof_purpose {
            ProofPurpose::AssertionMethod => {}
            ProofPurpose::Authentication => {}
            _ => return false,
        }

        let Ok(url) = DIDUrl::try_from(self.verification_method.id()) else {
            return false;
        };
        if url.id() != Some(subject) {
            return false;
        }

        signature::verify(data, &self.proof_value, &subject.to_signature_key()).is_ok()
    }
}

impl fmt::Display for Proof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Proof{{type={}, created={}, verificationMethod={}, proofPurpose={}, proofValue={}}}",
            self.proof_type,
            date::to_millis(self.created),
            self.verification_method,
            self.proof_purpose,
            hex::encode(&self.proof_value)
        )
    }
}

#[cfg(test)]
#[path = "unitests/test_proof.rs"]
mod test_proof;
