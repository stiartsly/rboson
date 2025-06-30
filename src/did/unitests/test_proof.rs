use std::time::{Duration, SystemTime};
use crate::{
    as_secs,
    signature,
};

use crate::did::{
    Proof,
    proof::{ProofType, ProofPurpose},
    VerificationMethod,
};

#[test]
fn test_proof_serde() {
    println!(">>as_sec:{}", as_secs!(SystemTime::now()));
    let created = SystemTime::UNIX_EPOCH + Duration::new(as_secs!(SystemTime::now()), 0);
    println!("created1: {:?}", created);

    let proof = Proof::new(
        ProofType::Ed25519Signature2020,
        created,
        VerificationMethod::reference("did:boson:1234567890".into()),
        ProofPurpose::AssertionMethod,
        vec![0u8; signature::Signature::BYTES]
    );
    assert_eq!(proof.types(), ProofType::Ed25519Signature2020);
    //assert_eq!(proof.created(), created);
    println!("proof.created(): {:?}", proof.created());
    println!("created: {:?}", created);
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
