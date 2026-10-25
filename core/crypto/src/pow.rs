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

//! The registration proof of work, as the Java SDK's `io.bosonnetwork.crypto.pow` (see the
//! Director's RegistrationPoW document): an Equihash puzzle bound to a super node and an identity,
//! plus an effort target on top of it.
//!
//! - [`equihash`]: the memory-hard puzzle (Java `Equihash`).
//! - [`proof_of_work`]: the puzzle seed, effort and signing message (Java `ProofOfWork`).
//! - [`registration`]: what a client solves and signs for a registration request (Java
//!   `RegistrationPowClient`).
//!
//! The challenge token itself (Java `PowChallenge`) is opaque to clients: they echo it back as received.

/// A self-contained Equihash puzzle: finding a solution is memory-hard, verifying one is cheap.
///
/// The leaf string for index `i` is the high `n` bits of
/// `Blake2b(personal = "BosonPoW" || n_le32 || k_le32, message = input || i_be32)`. A solution is
/// `2^k` distinct indices forming a tree whose leaves XOR to zero, each depth-`d` subtree zeroing its
/// leading `d * L` bits (`L = n / (k + 1)`), and the left subtree's first index below the right's.
pub mod equihash {
    use blake2b_simd::Params;

    use crate::{errors::ArgumentError, Result};

    const PERSONAL_PREFIX: &[u8; 8] = b"BosonPoW";

    // Guard against pathological memory growth during solving. Well-chosen parameters keep the
    // intermediate list near its initial size; this cap only trips on misuse.
    const MAX_LIST_SIZE: usize = 1 << 22;

    struct Entry {
        bits: Vec<u8>,
        indices: Vec<u32>,
    }

    /// Validates the parameters and returns the collision bit length `L = n / (k + 1)`.
    pub fn collision_bits(n: u32, k: u32) -> Result<u32> {
        if !(1..=20).contains(&k) {
            return Err(ArgumentError::new(format!("Invalid Equihash k: {k}")));
        }
        if !(2..=512).contains(&n) {
            return Err(ArgumentError::new(format!("Invalid Equihash n: {n}")));
        }
        if n % (k + 1) != 0 {
            return Err(ArgumentError::new("Equihash n must be divisible by (k + 1)"));
        }
        let l = n / (k + 1);
        if !(1..=30).contains(&l) {
            return Err(ArgumentError::new(format!("Equihash collision length out of range: {l}")));
        }
        Ok(l)
    }

    /// The number of leaf indices in a solution: `2^k`.
    pub fn solution_length(k: u32) -> usize {
        1 << k
    }

    /// The `n`-bit leaf string for one index: `ceil(n / 8)` bytes, the low bits past `n` zeroed.
    pub fn leaf(input: &[u8], index: u32, n: u32, k: u32) -> Vec<u8> {
        let hash_bytes = n.div_ceil(8) as usize;
        let mut personalization = [0u8; 16];
        personalization[..8].copy_from_slice(PERSONAL_PREFIX);
        personalization[8..12].copy_from_slice(&n.to_le_bytes());
        personalization[12..].copy_from_slice(&k.to_le_bytes());
        let mut output = Params::new()
            .hash_length(hash_bytes)
            .personal(&personalization)
            .to_state()
            .update(input)
            .update(&index.to_be_bytes())
            .finalize()
            .as_bytes()
            .to_vec();
        if n % 8 != 0 {
            let last = output.len() - 1;
            output[last] &= 0xff << (8 - n % 8);
        }
        output
    }

    /// Verifies a solution: `2^k` indices in solution order. Invalid parameters verify nothing.
    pub fn verify(input: &[u8], n: u32, k: u32, indices: &[u32]) -> bool {
        let Ok(l) = collision_bits(n, k) else {
            return false;
        };
        let count = solution_length(k);
        if indices.len() != count {
            return false;
        }
        let index_bound = 1u32 << (l + 1);
        let mut seen = vec![false; index_bound as usize];
        for &index in indices {
            if index >= index_bound || seen[index as usize] {
                return false; // out of range or duplicate
            }
            seen[index as usize] = true;
        }
        verify_node(input, n, k, l, indices).is_some_and(|root| is_all_zero(&root))
    }

    // Verifies a subtree of indices and returns its XOR string, or None if the ordering or collision
    // conditions fail.
    fn verify_node(input: &[u8], n: u32, k: u32, l: u32, indices: &[u32]) -> Option<Vec<u8>> {
        let width = indices.len();
        if width == 1 {
            return Some(leaf(input, indices[0], n, k));
        }
        let mid = width / 2;
        // Ordering: the left subtree's first index must be strictly less than the right's.
        if indices[0] >= indices[mid] {
            return None;
        }
        let left = verify_node(input, n, k, l, &indices[..mid])?;
        let right = verify_node(input, n, k, l, &indices[mid..])?;
        let x = xor(&left, &right);
        // A subtree covering 2^d leaves must have its leading d * L bits zeroed.
        let zeroed = width.trailing_zeros() * l;
        if zeroed < n && !has_leading_zero_bits(&x, zeroed) {
            return None;
        }
        Some(x)
    }

    /// Finds solutions with Wagner's algorithm: zero, one or several, each in canonical order and
    /// passing [`verify`]. Finds the same solutions, in the same order, as the Java SDK.
    pub fn solve(input: &[u8], n: u32, k: u32) -> Result<Vec<Vec<u32>>> {
        let l = collision_bits(n, k)?;
        let list_size = 1usize
            .checked_shl(l + 1)
            .filter(|size| *size <= MAX_LIST_SIZE)
            .ok_or_else(|| ArgumentError::new("Equihash parameter list is too large"))?;
        let mut entries = (0..list_size)
            .map(|index| Entry {
                bits: leaf(input, index as u32, n, k),
                indices: vec![index as u32],
            })
            .collect::<Vec<_>>();
        let mut solutions = Vec::new();

        for round in 1..=k {
            let start_bit = (round - 1) * l;
            // A stable sort, as Java's List.sort, so entries collide in the same order there.
            entries.sort_by_key(|entry| extract_bits(&entry.bits, start_bit, l));
            let mut next = Vec::with_capacity(list_size);
            let mut group_start = 0;
            while group_start < entries.len() {
                let key = extract_bits(&entries[group_start].bits, start_bit, l);
                let mut group_end = group_start + 1;
                while group_end < entries.len()
                    && extract_bits(&entries[group_end].bits, start_bit, l) == key
                {
                    group_end += 1;
                }
                // Collide every disjoint pair within the group.
                for left in group_start..group_end {
                    for right in left + 1..group_end {
                        if !disjoint(&entries[left].indices, &entries[right].indices) {
                            continue;
                        }
                        let bits = xor(&entries[left].bits, &entries[right].bits);
                        let indices = merge_canonical(&entries[left].indices, &entries[right].indices);
                        if round == k {
                            if is_all_zero(&bits) {
                                solutions.push(indices);
                            }
                        } else {
                            next.push(Entry { bits, indices });
                            if next.len() > MAX_LIST_SIZE {
                                return Err(ArgumentError::new("Equihash intermediate list overflow"));
                            }
                        }
                    }
                }
                group_start = group_end;
            }
            entries = next;
        }
        Ok(solutions)
    }

    /// Encodes a solution as big-endian 32-bit indices, the form the effort hash and the
    /// registration request use.
    pub fn encode_solution(indices: &[u32]) -> Vec<u8> {
        indices.iter().flat_map(|index| index.to_be_bytes()).collect()
    }

    /// Decodes a solution encoded by [`encode_solution`].
    pub fn decode_solution(bytes: &[u8]) -> Result<Vec<u32>> {
        if bytes.len() % 4 != 0 {
            return Err(ArgumentError::new(format!(
                "Invalid solution encoding length: {}",
                bytes.len()
            )));
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .collect())
    }

    // Extracts length bits (length <= 32) starting at start_bit, bit 0 being the MSB of byte 0.
    fn extract_bits(bytes: &[u8], start_bit: u32, length: u32) -> u32 {
        (start_bit..start_bit + length).fold(0, |value, bit| {
            (value << 1) | u32::from((bytes[(bit / 8) as usize] >> (7 - bit % 8)) & 1)
        })
    }

    // Whether the first count bits (from the MSB) are all zero.
    fn has_leading_zero_bits(bytes: &[u8], count: u32) -> bool {
        let full_bytes = (count / 8) as usize;
        if bytes[..full_bytes].iter().any(|&b| b != 0) {
            return false;
        }
        let rem = count % 8;
        rem == 0 || bytes[full_bytes] & (0xff << (8 - rem)) == 0
    }

    fn is_all_zero(bytes: &[u8]) -> bool {
        bytes.iter().all(|&b| b == 0)
    }

    fn xor(left: &[u8], right: &[u8]) -> Vec<u8> {
        left.iter().zip(right).map(|(a, b)| a ^ b).collect()
    }

    fn disjoint(left: &[u32], right: &[u32]) -> bool {
        !left.iter().any(|index| right.contains(index))
    }

    // Concatenates the two index lists so the one with the smaller first index comes first.
    fn merge_canonical(left: &[u32], right: &[u32]) -> Vec<u32> {
        let (first, second) = if left[0] < right[0] { (left, right) } else { (right, left) };
        first.iter().chain(second).copied().collect()
    }
}

/// The registration proof of work: an [`equihash`] puzzle bound to a super node and an identity,
/// with an effort target.
///
/// `seed = superNodeId(32) || pubkey(32) || challengeNonce(32)`; each attempt solves the puzzle for
/// `seed || powNonce(8, big-endian)`, and a solution counts when
/// `Blake2b-256(seed || powNonce || encode(solution))` has at least `effort` leading zero bits.
pub mod proof_of_work {
    use blake2b_simd::Params;

    use super::equihash;
    use crate::{errors::ArgumentError, Result};

    /// The byte length of each identifier that composes the seed.
    pub const ID_BYTES: usize = 32;
    /// The byte length of the seed: `superNodeId || pubkey || challengeNonce`.
    pub const SEED_BYTES: usize = ID_BYTES * 3;
    /// The byte length of the effort hash (Blake2b-256).
    pub const EFFORT_HASH_BYTES: usize = 32;

    /// A proof of work found: the winning nonce and its Equihash solution.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Solution {
        pub pow_nonce: u64,
        pub indices: Vec<u32>,
    }

    /// The puzzle seed for a super node, an identity's public key and a challenge nonce.
    pub fn seed(super_node_id: &[u8], pubkey: &[u8], challenge_nonce: &[u8]) -> Result<[u8; SEED_BYTES]> {
        require_length(super_node_id, "superNodeId")?;
        require_length(pubkey, "pubkey")?;
        require_length(challenge_nonce, "challengeNonce")?;
        let mut seed = [0u8; SEED_BYTES];
        seed[..ID_BYTES].copy_from_slice(super_node_id);
        seed[ID_BYTES..ID_BYTES * 2].copy_from_slice(pubkey);
        seed[ID_BYTES * 2..].copy_from_slice(challenge_nonce);
        Ok(seed)
    }

    // The per-attempt Equihash input: seed || powNonce (8 bytes, big-endian).
    fn puzzle_input(seed: &[u8], pow_nonce: u64) -> Vec<u8> {
        let mut input = Vec::with_capacity(seed.len() + 8);
        input.extend_from_slice(seed);
        input.extend_from_slice(&pow_nonce.to_be_bytes());
        input
    }

    /// The message an identity signs to bind its key to a solution:
    /// `superNodeId || pubkey || challengeNonce || powNonce || effort(4, big-endian)`.
    pub fn signing_message(
        super_node_id: &[u8],
        pubkey: &[u8],
        challenge_nonce: &[u8],
        pow_nonce: &[u8],
        effort: u32,
    ) -> Vec<u8> {
        [super_node_id, pubkey, challenge_nonce, pow_nonce, &effort.to_be_bytes()].concat()
    }

    /// A nonce as 8 big-endian bytes, its wire form.
    pub fn encode_nonce(pow_nonce: u64) -> [u8; 8] {
        pow_nonce.to_be_bytes()
    }

    /// The effort hash of a candidate: `Blake2b-256(seed || powNonce || encode(solution))`.
    pub fn effort_hash(seed: &[u8], pow_nonce: u64, indices: &[u32]) -> [u8; EFFORT_HASH_BYTES] {
        Params::new()
            .hash_length(EFFORT_HASH_BYTES)
            .to_state()
            .update(&puzzle_input(seed, pow_nonce))
            .update(&equihash::encode_solution(indices))
            .finalize()
            .as_bytes()
            .try_into()
            .expect("the Blake2b hash length is fixed")
    }

    /// Whether a hash has at least `effort` leading zero bits.
    pub fn meets_effort(hash: &[u8], effort: u32) -> bool {
        leading_zero_bits(hash) >= effort
    }

    /// The number of leading zero bits of `hash`, big-endian.
    pub fn leading_zero_bits(hash: &[u8]) -> u32 {
        let mut count = 0;
        for &byte in hash {
            count += byte.leading_zeros();
            if byte != 0 {
                break;
            }
        }
        count
    }

    /// Searches for a proof of work meeting both the puzzle and the effort target, trying up to
    /// `max_pow_nonces` nonces. This is the memory-hard, slow part. `None` when the budget runs out.
    pub fn solve(seed: &[u8], n: u32, k: u32, effort: u32, max_pow_nonces: u64) -> Result<Option<Solution>> {
        if seed.len() != SEED_BYTES {
            return Err(ArgumentError::new(format!(
                "seed must be {SEED_BYTES} bytes, got {}",
                seed.len()
            )));
        }
        equihash::collision_bits(n, k)?; // validate the parameters early
        for pow_nonce in 0..max_pow_nonces {
            for indices in equihash::solve(&puzzle_input(seed, pow_nonce), n, k)? {
                if meets_effort(&effort_hash(seed, pow_nonce, &indices), effort) {
                    return Ok(Some(Solution { pow_nonce, indices }));
                }
            }
        }
        Ok(None)
    }

    /// Verifies a proof of work: a valid Equihash solution for `seed || powNonce` whose effort hash
    /// meets the effort target.
    pub fn verify(seed: &[u8], n: u32, k: u32, effort: u32, pow_nonce: u64, indices: &[u32]) -> bool {
        seed.len() == SEED_BYTES
            && equihash::verify(&puzzle_input(seed, pow_nonce), n, k, indices)
            && meets_effort(&effort_hash(seed, pow_nonce, indices), effort)
    }

    fn require_length(value: &[u8], name: &str) -> Result<()> {
        if value.len() != ID_BYTES {
            return Err(ArgumentError::new(format!(
                "{name} must be {ID_BYTES} bytes, got {}",
                value.len()
            )));
        }
        Ok(())
    }
}

/// The client side of the registration proof of work: solve a challenge and sign the result, then
/// submit the fields with the registration request.
pub mod registration {
    use super::proof_of_work;
    use crate::{errors::CryptoError, signature::KeyPair, Result};

    /// The solved and signed fields a client attaches to its registration request.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Proof {
        /// The proof-of-work nonce, 8 bytes big-endian.
        pub pow_nonce: [u8; 8],
        /// The Equihash solution indices.
        pub solution: Vec<u32>,
        /// The identity's signature over the registration signing message.
        pub signature: Vec<u8>,
    }

    /// Solves the challenge for an identity and signs the result with its key. Fails when no proof
    /// is found within `max_pow_nonces` nonces.
    #[allow(clippy::too_many_arguments)]
    pub fn solve(
        super_node_id: &[u8],
        signer: &KeyPair,
        n: u32,
        k: u32,
        effort: u32,
        challenge_nonce: &[u8],
        max_pow_nonces: u64,
    ) -> Result<Proof> {
        let pubkey = signer.public_key().as_bytes();
        let seed = proof_of_work::seed(super_node_id, pubkey, challenge_nonce)?;
        let solution = proof_of_work::solve(&seed, n, k, effort, max_pow_nonces)?
            .ok_or_else(|| CryptoError::new("No proof-of-work found within the nonce budget"))?;
        let pow_nonce = proof_of_work::encode_nonce(solution.pow_nonce);
        let signature = sign(super_node_id, signer, challenge_nonce, &pow_nonce, effort)?;
        Ok(Proof {
            pow_nonce,
            solution: solution.indices,
            signature,
        })
    }

    /// Signs the registration message with a key that co-signs a solution already found (for example
    /// the initial device key). The message binds the signer's own public key.
    pub fn sign(
        super_node_id: &[u8],
        signer: &KeyPair,
        challenge_nonce: &[u8],
        pow_nonce: &[u8],
        effort: u32,
    ) -> Result<Vec<u8>> {
        let message = proof_of_work::signing_message(
            super_node_id,
            signer.public_key().as_bytes(),
            challenge_nonce,
            pow_nonce,
            effort,
        );
        signer.private_key().sign_into(&message)
    }
}
