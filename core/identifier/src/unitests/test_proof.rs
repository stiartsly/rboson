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

use boson_api::{
    as_secs,
    signature,
};
use crate::{
    {
        proof::{ProofPurpose, ProofType},
        Proof, VerificationMethod,
    },
};
use std::time::{Duration, SystemTime};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_serde() {
        let created = SystemTime::UNIX_EPOCH + Duration::new(as_secs!(SystemTime::now()), 0);
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            created,
            VerificationMethod::reference("did:boson:1234567890".into()),
            ProofPurpose::AssertionMethod,
            vec![0u8; signature::Signature::BYTES],
        );
        assert_eq!(proof.types(), ProofType::Ed25519Signature2020);
        assert_eq!(proof.created(), created);
        assert_eq!(proof.verification_method().id(), "did:boson:1234567890");
        assert_eq!(proof.purpose(), ProofPurpose::AssertionMethod);
        assert_eq!(proof.proof_value().len(), signature::Signature::BYTES);

        let json = serde_json::to_string(&proof).unwrap();
        println!("serded json: {}", json);
        println!("proof: {}", proof);

        let rc = serde_json::from_str::<Proof>(&json);
        assert!(rc.is_ok());

        let proof2 = rc.unwrap();
        assert_eq!(proof, proof2);
    }

    #[test]
    fn test_proof_verify() {
        // TODO
        assert!(true); // Placeholder for actual verification test
    }
}
