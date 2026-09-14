use blake2b_simd::Params;

use crate::{errors::ArgumentError, signature::KeyPair, Id, Result};

const MAX_LIST_SIZE: usize = 1 << 22;

pub(super) const MAX_POW_NONCES: u64 = 1_000_000;

pub(super) struct Solution {
    pub pow_nonce: [u8; 8],
    pub indices: Vec<u32>,
    pub signature: Vec<u8>,
}

pub(super) fn solve(
    node_id: Id,
    key: &KeyPair,
    n: u32,
    k: u32,
    effort: u32,
    challenge_nonce: &[u8; 32],
) -> Result<Solution> {
    let mut seed = Vec::with_capacity(96);
    seed.extend_from_slice(node_id.as_bytes());
    seed.extend_from_slice(key.public_key().as_bytes());
    seed.extend_from_slice(challenge_nonce);

    let (pow_nonce, indices) = solve_equihash(&seed, n, k, effort)?;
    let signature = sign(node_id, key, challenge_nonce, &pow_nonce, effort)?;
    Ok(Solution {
        pow_nonce,
        indices,
        signature,
    })
}

pub(super) fn sign(
    node_id: Id,
    key: &KeyPair,
    challenge_nonce: &[u8; 32],
    pow_nonce: &[u8; 8],
    effort: u32,
) -> Result<Vec<u8>> {
    let mut message = Vec::with_capacity(108);
    message.extend_from_slice(node_id.as_bytes());
    message.extend_from_slice(key.public_key().as_bytes());
    message.extend_from_slice(challenge_nonce);
    message.extend_from_slice(pow_nonce);
    message.extend_from_slice(&effort.to_be_bytes());
    key.private_key().sign_into(&message)
}

fn solve_equihash(seed: &[u8], n: u32, k: u32, effort: u32) -> Result<([u8; 8], Vec<u32>)> {
    let collision_bits = validate_parameters(n, k)?;
    for nonce in 0..MAX_POW_NONCES {
        let pow_nonce = nonce.to_be_bytes();
        let mut input = Vec::with_capacity(seed.len() + pow_nonce.len());
        input.extend_from_slice(seed);
        input.extend_from_slice(&pow_nonce);
        for indices in equihash_solutions(&input, n, k, collision_bits)? {
            if leading_zero_bits(&effort_hash(&input, &indices)) >= effort {
                return Ok((pow_nonce, indices));
            }
        }
    }
    Err(ArgumentError::new(
        "No proof-of-work found within the nonce budget",
    ))
}

#[derive(Clone)]
struct Entry {
    bits: Vec<u8>,
    indices: Vec<u32>,
}

fn equihash_solutions(input: &[u8], n: u32, k: u32, collision_bits: u32) -> Result<Vec<Vec<u32>>> {
    let list_size = 1usize
        .checked_shl(collision_bits + 1)
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
        let start_bit = (round - 1) * collision_bits;
        entries.sort_unstable_by_key(|entry| extract_bits(&entry.bits, start_bit, collision_bits));
        let mut next = Vec::with_capacity(list_size);
        let mut group_start = 0;
        while group_start < entries.len() {
            let key = extract_bits(&entries[group_start].bits, start_bit, collision_bits);
            let mut group_end = group_start + 1;
            while group_end < entries.len()
                && extract_bits(&entries[group_end].bits, start_bit, collision_bits) == key
            {
                group_end += 1;
            }
            for left in group_start..group_end {
                for right in left + 1..group_end {
                    if !disjoint(&entries[left].indices, &entries[right].indices) {
                        continue;
                    }
                    let bits = xor(&entries[left].bits, &entries[right].bits);
                    let indices = merge_canonical(&entries[left].indices, &entries[right].indices);
                    if round == k {
                        if bits.iter().all(|&byte| byte == 0) {
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

fn validate_parameters(n: u32, k: u32) -> Result<u32> {
    if !(1..=20).contains(&k) || !(2..=512).contains(&n) || n % (k + 1) != 0 {
        return Err(ArgumentError::new("Invalid Equihash parameters"));
    }
    let collision_bits = n / (k + 1);
    if !(1..=30).contains(&collision_bits) {
        return Err(ArgumentError::new("Equihash collision length out of range"));
    }
    Ok(collision_bits)
}

fn leaf(input: &[u8], index: u32, n: u32, k: u32) -> Vec<u8> {
    let hash_bytes = n.div_ceil(8) as usize;
    let mut personalization = [0u8; 16];
    personalization[..8].copy_from_slice(b"BosonPoW");
    personalization[8..12].copy_from_slice(&n.to_le_bytes());
    personalization[12..].copy_from_slice(&k.to_le_bytes());
    let mut message = Vec::with_capacity(input.len() + 4);
    message.extend_from_slice(input);
    message.extend_from_slice(&index.to_be_bytes());
    let mut output = Params::new()
        .hash_length(hash_bytes)
        .personal(&personalization)
        .hash(&message)
        .as_bytes()
        .to_vec();
    if !n.is_multiple_of(8) {
        let last = output.len() - 1;
        output[last] &= 0xff << (8 - n % 8);
    }
    output
}

fn effort_hash(input: &[u8], indices: &[u32]) -> [u8; 32] {
    let mut message = Vec::with_capacity(input.len() + indices.len() * 4);
    message.extend_from_slice(input);
    for index in indices {
        message.extend_from_slice(&index.to_be_bytes());
    }
    Params::new()
        .hash_length(32)
        .hash(&message)
        .as_bytes()
        .try_into()
        .expect("Blake2b hash length is fixed")
}

fn leading_zero_bits(hash: &[u8]) -> u32 {
    hash.iter()
        .map(|byte| byte.leading_zeros())
        .scan(true, |leading, zeros| {
            if *leading && zeros == 8 {
                Some(8)
            } else if *leading {
                *leading = false;
                Some(zeros)
            } else {
                Some(0)
            }
        })
        .sum()
}

fn extract_bits(bytes: &[u8], start_bit: u32, length: u32) -> u32 {
    (start_bit..start_bit + length).fold(0, |value, bit| {
        (value << 1) | u32::from((bytes[(bit / 8) as usize] >> (7 - bit % 8)) & 1)
    })
}

fn xor(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter()
        .zip(right)
        .map(|(left, right)| left ^ right)
        .collect()
}

fn disjoint(left: &[u32], right: &[u32]) -> bool {
    !left.iter().any(|index| right.contains(index))
}

fn merge_canonical(left: &[u32], right: &[u32]) -> Vec<u32> {
    let (first, second) = if left[0] < right[0] {
        (left, right)
    } else {
        (right, left)
    };
    first.iter().chain(second).copied().collect()
}
