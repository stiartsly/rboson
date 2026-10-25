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

// Ported from the Java SDK's EquihashTests, ProofOfWorkTests and RegistrationPowClientTests.

use crate::{
    pow::{equihash, proof_of_work, registration},
    random::secure_bytes,
    signature::KeyPair,
};

// Small but valid parameters that keep solving fast: L = 48 / 4 = 12, index space 2^13.
const N: u32 = 48;
const K: u32 = 3;

// An input the solver finds at least one solution for. The solution count of a random input is
// Poisson-distributed with a mean of 2 here, so one draw finds nothing about 15% of the time.
fn solvable_puzzle() -> (Vec<u8>, Vec<Vec<u32>>) {
    for _ in 0..32 {
        let input = secure_bytes(104);
        let solutions = equihash::solve(&input, N, K).unwrap();
        if !solutions.is_empty() {
            return (input, solutions);
        }
    }
    panic!("no solvable input found in 32 attempts");
}

fn random_seed() -> [u8; proof_of_work::SEED_BYTES] {
    proof_of_work::seed(&secure_bytes(32), &secure_bytes(32), &secure_bytes(32)).unwrap()
}

#[test]
fn collision_bits_and_params() {
    assert_eq!(equihash::collision_bits(N, K).unwrap(), 12);
    assert_eq!(equihash::solution_length(K), 8);
    assert!(equihash::collision_bits(50, K).is_err()); // 50 % 4 != 0
    assert!(equihash::collision_bits(N, 0).is_err());
}

#[test]
fn solve_produces_verifiable_solutions() {
    let (input, solutions) = solvable_puzzle();
    for solution in &solutions {
        assert_eq!(solution.len(), equihash::solution_length(K));
        assert!(equihash::verify(&input, N, K, solution), "solver output must verify");
    }
}

#[test]
fn tampered_solution_fails_verification() {
    let (input, solutions) = solvable_puzzle();
    let mut solution = solutions[0].clone();
    // Flip one index to a different in-range value: breaks the collision or ordering.
    solution[0] ^= 1;
    assert!(!equihash::verify(&input, N, K, &solution));
    // Wrong length.
    assert!(!equihash::verify(&input, N, K, &solutions[0][..solutions[0].len() - 1]));
    // Duplicated index.
    let mut dup = solutions[0].clone();
    let last = dup.len() - 1;
    dup[last] = dup[0];
    assert!(!equihash::verify(&input, N, K, &dup));
}

#[test]
fn solution_bound_to_input() {
    let (mut input, solutions) = solvable_puzzle();
    input[0] ^= 0x01;
    assert!(!equihash::verify(&input, N, K, &solutions[0]), "a solution must not verify against a different input");
}

#[test]
fn solution_encoding_roundtrip() {
    let indices = [1, 2, 3, 100, 200, 4095, 8000, 8191];
    let encoded = equihash::encode_solution(&indices);
    assert_eq!(encoded.len(), indices.len() * 4);
    assert_eq!(equihash::decode_solution(&encoded).unwrap(), indices);
    assert!(equihash::decode_solution(&encoded[1..]).is_err());
}

#[test]
fn leading_zero_bits() {
    assert_eq!(proof_of_work::leading_zero_bits(&[0xff]), 0);
    assert_eq!(proof_of_work::leading_zero_bits(&[0x00, 0xff]), 8);
    assert_eq!(proof_of_work::leading_zero_bits(&[0x00, 0x00, 0x40]), 17);
    assert_eq!(proof_of_work::leading_zero_bits(&[0x00, 0x00, 0x00]), 24);
}

#[test]
fn seed_layout() {
    let (node, pubkey, nonce) = (secure_bytes(32), secure_bytes(32), secure_bytes(32));
    let seed = proof_of_work::seed(&node, &pubkey, &nonce).unwrap();
    assert_eq!(seed.len(), proof_of_work::SEED_BYTES);
    assert_eq!(&seed[..32], node.as_slice());
    assert_eq!(&seed[64..], nonce.as_slice());
    assert!(proof_of_work::seed(&node[..31], &pubkey, &nonce).is_err());
}

#[test]
fn solve_and_verify_roundtrip() {
    let seed = random_seed();
    let solution = proof_of_work::solve(&seed, N, K, 0, 512).unwrap().expect("should find a solution at effort 0");
    assert!(proof_of_work::verify(&seed, N, K, 0, solution.pow_nonce, &solution.indices));
}

#[test]
fn verify_enforces_effort() {
    let seed = random_seed();
    let solution = proof_of_work::solve(&seed, N, K, 0, 512).unwrap().unwrap();
    let actual = proof_of_work::leading_zero_bits(&proof_of_work::effort_hash(&seed, solution.pow_nonce, &solution.indices));
    // The found solution satisfies effort = actual but not effort = actual + 1.
    assert!(proof_of_work::verify(&seed, N, K, actual, solution.pow_nonce, &solution.indices));
    assert!(!proof_of_work::verify(&seed, N, K, actual + 1, solution.pow_nonce, &solution.indices));
}

#[test]
fn verify_rejects_tampering() {
    let seed = random_seed();
    let solution = proof_of_work::solve(&seed, N, K, 0, 512).unwrap().unwrap();
    // Wrong nonce.
    assert!(!proof_of_work::verify(&seed, N, K, 0, solution.pow_nonce + 1, &solution.indices));
    // Wrong seed.
    let mut other = seed;
    other[0] ^= 0x01;
    assert!(!proof_of_work::verify(&other, N, K, 0, solution.pow_nonce, &solution.indices));
}

#[test]
fn solve_honours_effort_target() {
    let seed = random_seed();
    let effort = 5;
    let solution = proof_of_work::solve(&seed, N, K, effort, 4096).unwrap().expect("should meet a small effort target");
    assert!(proof_of_work::meets_effort(&proof_of_work::effort_hash(&seed, solution.pow_nonce, &solution.indices), effort));
}

#[test]
fn registration_produces_server_verifiable_fields() {
    let super_node_id = KeyPair::random().public_key().as_bytes().to_vec();
    let user_key = KeyPair::random();
    let challenge_nonce = secure_bytes(32);
    let effort = 0;
    let proof = registration::solve(&super_node_id, &user_key, N, K, effort, &challenge_nonce, 512).unwrap();

    // Server side: the proof of work verifies.
    let seed = proof_of_work::seed(&super_node_id, user_key.public_key().as_bytes(), &challenge_nonce).unwrap();
    assert!(proof_of_work::verify(&seed, N, K, effort, u64::from_be_bytes(proof.pow_nonce), &proof.solution));
    // Server side: the identity's signature over the registration message verifies.
    let message = proof_of_work::signing_message(&super_node_id, user_key.public_key().as_bytes(), &challenge_nonce, &proof.pow_nonce, effort);
    assert!(user_key.public_key().verify(&message, &proof.signature).unwrap());
}

#[test]
fn additional_device_signature_verifies() {
    let super_node_id = KeyPair::random().public_key().as_bytes().to_vec();
    let (user_key, device_key) = (KeyPair::random(), KeyPair::random());
    let challenge_nonce = secure_bytes(32);
    let proof = registration::solve(&super_node_id, &user_key, N, K, 0, &challenge_nonce, 512).unwrap();
    let device_sig = registration::sign(&super_node_id, &device_key, &challenge_nonce, &proof.pow_nonce, 0).unwrap();
    // The device key signs a message bound to its own identity, verified against the device key.
    let message = proof_of_work::signing_message(&super_node_id, device_key.public_key().as_bytes(), &challenge_nonce, &proof.pow_nonce, 0);
    assert!(device_key.public_key().verify(&message, &device_sig).unwrap());
}

#[test]
fn no_proof_within_the_budget_is_an_error() {
    let super_node_id = secure_bytes(32);
    assert!(registration::solve(&super_node_id, &KeyPair::random(), N, K, 255, &secure_bytes(32), 2).is_err());
}
