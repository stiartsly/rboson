use std::fmt;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};

use crate::{
    as_secs,
    Id,
    signature,
};

use super::{
    VerificationMethod,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
pub enum ProofPurpose {
    AssertionMethod,
    Authentication,
    CapabilityInvocation,
    CapabilityDelegation,
}

impl fmt::Display for ProofPurpose {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ProofPurpose::AssertionMethod       => "AssertionMethod",
            ProofPurpose::Authentication        => "Authentication",
            ProofPurpose::CapabilityInvocation  => "CapabilityInvocation",
            ProofPurpose::CapabilityDelegation  => "CapabilityDelegation",
        })
    }
}

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Proof {
    #[serde(rename = "type")]
    proof_type: ProofType,

    #[serde(rename = "created")]
    created: u64,

    #[serde(rename = "verificationMethod")]
    verification_method: VerificationMethod,

    #[serde(rename = "proofPurpose")]
    proof_purpose: ProofPurpose,

    #[serde(rename = "proofValue", skip_serializing_if = "Vec::is_empty")]
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
            created: as_secs!(created),
            verification_method,
            proof_purpose,
            proof_value,
        }
    }

    pub fn types(&self) -> ProofType {
        self.proof_type
    }

    pub fn created(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.created)
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
            return false ;
        }

        if self.proof_purpose != ProofPurpose::AssertionMethod &&
            self.proof_purpose != ProofPurpose::Authentication {
            return false;
        }

        /*let url = DIDUrl::from_id(self.verification_method.id());
        if url.id() != subject {
            return false;
        }*/

        //if self.verification_method.id() != subject {
        //    return false;
        //}

        signature::verify(
            data,
            &self.proof_value,
            &subject.to_signature_key()
        ).is_ok()
    }
}

impl PartialEq<Self> for Proof {
    fn eq(&self, other: &Self) -> bool {
        self.proof_type == other.proof_type &&
        self.created == other.created &&
        self.verification_method == other.verification_method &&
        self.proof_purpose == other.proof_purpose &&
        self.proof_value == other.proof_value
    }
}

impl fmt::Display for Proof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "Proof{{type={},created={},verificationMethod:{:?},proofPurpose={},proofValue={}}}",
            self.proof_type,
            self.created,
            self.verification_method,
            self.proof_purpose,
            hex::encode(&self.proof_value)
        )
    }
}
