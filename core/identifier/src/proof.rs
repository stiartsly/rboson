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
