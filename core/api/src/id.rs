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

use std::{
    fmt, ops, str,
    cmp::Ordering,
    result::Result as StdResult,
};
use bs58;
use serde::{
    de::{self, Deserializer, Visitor},
    ser::Serializer,
    Deserialize,
    Serialize,
};
use crate::{
    cryptobox,
    signature,
    errors::{Error, Result, ArgumentError}
};

pub const DID_PREFIX: &str = "did:boson:";

#[derive(Debug, Copy, Clone, Default, PartialOrd, PartialEq, Ord, Eq, Hash)]
pub struct Id([u8; Id::BYTES]);

impl Id {
    pub const BYTES: usize = 32;
    pub const BITS: usize = 256;

    pub const MAX_ID: Id = Id::max();
    pub const MIN_ID: Id = Id::min();
    pub const ZERO_ID: Id = Id::MIN_ID;

    pub fn random() -> Self {
        Id(crate::random_array::<{ Self::BYTES }>())
    }

    pub fn generate() -> Self {
        Self::random()
    }

    pub fn from_bytes(input: [u8; Id::BYTES]) -> Self {
        Id(input)
    }

    pub fn try_from_bytes(input: &[u8]) -> Result<Self> {
        if input.len() != Id::BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid bytes length {} for ID, expected length {}",
                input.len(),
                Id::BYTES
            )));
        }
        let mut bytes = [0u8; Id::BYTES];
        bytes.copy_from_slice(input);
        Ok(Id(bytes))
    }

    pub fn try_from_bytes_at(input: &[u8], offset: usize) -> Result<Self> {
        let end = offset
            .checked_add(Self::BYTES)
            .ok_or_else(|| ArgumentError::new("ID byte offset is out of range"))?;
        let bytes = input
            .get(offset..end)
            .ok_or_else(|| ArgumentError::new("ID byte offset is out of range"))?;
        Self::try_from_bytes(bytes)
    }

    /// Parses hexadecimal text with or without a `0x`/`0X` prefix.
    pub fn try_from_hexstr(input: &str) -> Result<Self> {
        let input = input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
            .unwrap_or(input);
        if input.len() != Self::BYTES * 2 {
            return Err(ArgumentError::new(format!(
                "Hexadecimal string must represent exactly {} bytes",
                Self::BYTES
            )));
        }

        let mut bytes = [0u8; Id::BYTES];
        hex::decode_to_slice(input, &mut bytes[..])
            .map_err(|e| ArgumentError::new(format!("Invalid hex format string: {e}")))?;
        Ok(Id(bytes))
    }

    pub fn try_from_base58(input: &str) -> Result<Self> {
        if input.starts_with("0x") || input.starts_with("0X") {
            return Err(ArgumentError::new(
                "Base58 format strings must not have a '0x' prefix.",
            ));
        }

        let mut bytes = [0u8; Id::BYTES];
        let decoded_len = bs58::decode(input)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .onto(&mut bytes[..])
            .map_err(|e| ArgumentError::new(format!("Invalid base58 format string: {e}")))?;

        if decoded_len != Id::BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid base58 byte length {decoded_len} for ID, expected {}",
                Id::BYTES
            )));
        }
        Ok(Id(bytes))
    }

    //  Creates an id with the specified bit set to 1.
    pub fn try_from_bit_at(index: usize) -> Result<Self> {
        if index >= Id::BITS {
            return Err(ArgumentError::new(format!(
                "Index {} is out of bounds for ID with {} bits",
                index,
                Id::BITS
            )));
        }

        let mut bytes = [0u8; Id::BYTES];
        let byte_index = index / 8;
        let bit_index = index % 8;

        bytes[byte_index] |= 1 << (7 - bit_index);
        Ok(Id(bytes))
    }

    pub const fn min() -> Self {
        Id([0x0; Id::BYTES])
    }

    pub const fn max() -> Self {
        Id([0xFF; Id::BYTES])
    }

    pub const fn zero() -> Self {
        Id([0u8; Id::BYTES])
    }

    pub fn to_hexstr(&self) -> String {
        format!("0x{}", hex::encode(&self.0))
    }

    pub fn to_abbr_hexstr(&self) -> String {
        let hex = hex::encode(&self.0);
        format!("0x{}...{}", &hex[..6], &hex[hex.len() - 4..])
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(self.0)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .into_string()
    }

    pub fn to_abbr_base58(&self) -> String {
        let bs58 = self.to_base58();
        format!("{}...{}", &bs58[..4], &bs58[bs58.len() - 4..])
    }

    pub fn to_did_string(&self) -> String {
        format!("{}{}", DID_PREFIX, self.to_base58())
    }

    /// Compact binary representation without separators.
    pub fn to_binary_string(&self) -> String {
        let mut out = String::with_capacity(Self::BITS);
        for byte in &self.0 {
            out.push_str(&format!("{byte:08b}"));
        }
        out
    }

    pub fn to_abbr_str(&self) -> String {
        self.to_abbr_base58()
    }

    /// The Ed25519 public key with this id's bytes.
    pub fn to_signature_key(&self) -> signature::PublicKey {
        signature::PublicKey::try_from(self.as_bytes()).expect("an id has the size of a public key")
    }

    /// The X25519 public key converted from this id's Ed25519 key. Fails when the id is not a valid
    /// Ed25519 point, as any 32 bytes from the network may not be.
    pub fn to_encryption_key(&self) -> Result<cryptobox::PublicKey> {
        cryptobox::PublicKey::try_from(&self.to_signature_key())
    }

    pub fn distance(&self, other: &Id) -> Id {
        let mut bytes = [0u8; Id::BYTES];
        for i in 0..Id::BYTES {
            bytes[i] = self.0[i] ^ other.0[i];
        }
        Id(bytes)
    }

    pub const fn size(&self) -> usize {
        Id::BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[allow(unused)]
    #[doc(hidden)]
    pub fn update(&mut self, cb: impl Fn(&mut [u8])) {
        cb(self.0.as_mut_slice());
    }

    /*
    Order::Less     : means a is closer to the target than b.
    Order::Greater  : means a is farther from the target than b.
    Order::Equal    : means a and b are equidistant from the target.
     */
    pub fn three_way_compare(&self, a: &Self, b: &Self) -> Ordering {
        let mut mmi = Id::BYTES;
        for i in 0..Id::BYTES {
            if a.0[i] != b.0[i] {
                mmi = i;
                break;
            }
        }
        if mmi == Id::BYTES {
            return Ordering::Equal;
        }

        let _a = a.0[mmi] ^ self.0[mmi];
        let _b = b.0[mmi] ^ self.0[mmi];

        _a.cmp(&_b)
    }

    pub fn distance_between(a: &Id, b: &Id) -> Id {
        a.distance(b)
    }

    pub fn bits_equal(a: &Id, b: &Id, depth: i32) -> bool {
        if depth < 0 || depth >= Self::BITS as i32 {
            return false;
        }

        let mut mmi = usize::MAX;
        for i in 0..Id::BYTES {
            if a.0[i] != b.0[i] {
                mmi = i;
                break;
            }
        }

        let idx = (depth >> 3) as usize;
        let diff: u8 = a.0[idx] ^ b.0[idx];
        // Create a bitmask with the lower n bits set
        let mask = (0xff80 >> (depth & 0x07)) as u8;
        // Use the bitmask to check if the lower bits are all zeros
        let is_diff = (diff & mask) == 0;

        match mmi == idx {
            true => is_diff,
            false => mmi > idx,
        }
    }

    #[doc(hidden)]
    pub fn bits_copy(src: &Id, dst: &mut Id, depth: i32) {
        if depth == -1 {
            return;
        }

        let idx = (depth >> 3) as usize;
        if idx > 0 {
            dst.0[..idx].copy_from_slice(&src.0[..idx]);
        }

        let mask = (0xff80 >> (depth & 0x07)) as u8;
        dst.0[idx] &= !mask;
        dst.0[idx] |= src.0[idx] & mask;
    }
}

impl Id {
    /// Reads an unsigned 32-bit big-endian integer at byte `offset`.
    pub fn get_int(&self, offset: usize) -> Result<u32> {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| ArgumentError::new("ID integer offset is out of range"))?;
        let bytes = self
            .0
            .get(offset..end)
            .ok_or_else(|| ArgumentError::new("ID integer offset is out of range"))?;
        Ok(u32::from_be_bytes(bytes.try_into().expect("four-byte slice")))
    }

    /// Adds two ids using 256-bit unsigned two's-complement arithmetic.
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0u8; Self::BYTES];
        let mut carry = 0u16;
        for index in (0..Self::BYTES).rev() {
            let sum = self.0[index] as u16 + other.0[index] as u16 + carry;
            result[index] = sum as u8;
            carry = sum >> 8;
        }
        Id(result)
    }

    /// Returns an id at `distance` bits from this one under the XOR metric.
    pub fn get_id_by_distance(&self, distance: usize) -> Result<Self> {
        if distance > Self::BITS {
            return Err(ArgumentError::new(format!(
                "Distance must be in range [0, {}]",
                Self::BITS
            )));
        }

        let mut difference = [0u8; Self::BYTES];
        let zero_bytes = (Self::BITS - distance) / 8;
        let zero_bits = (Self::BITS - distance) % 8;
        if zero_bytes < Self::BYTES {
            difference[zero_bytes] = 0xFFu8 >> zero_bits;
            difference[zero_bytes + 1..].fill(0xFF);
        }
        Ok(self.distance(&Id(difference)))
    }

    /// Approximate XOR distance: the number of significant bits in the distance.
    pub fn approx_distance(&self, other: &Self) -> usize {
        Self::BITS - self.distance(other).get_leading_zeros()
    }

    pub fn get_leading_zeros(&self) -> usize {
        let mut zeros = 0;
        for byte in self.0 {
            if byte == 0 {
                zeros += 8;
            } else {
                zeros += byte.leading_zeros() as usize;
                break;
            }
        }
        zeros
    }

    pub fn get_trailing_zeros(&self) -> usize {
        let mut zeros = 0;
        for byte in self.0.iter().rev() {
            if *byte == 0 {
                zeros += 8;
            } else {
                zeros += byte.trailing_zeros() as usize;
                break;
            }
        }
        zeros
    }

    /// Lexicographic unsigned comparison.
    pub fn compare(left: &Self, right: &Self) -> Ordering {
        left.0.cmp(&right.0)
    }

    /// Minimum id derived from the all-zero private key (test/invalid edge case).
    pub fn min_private_key_id() -> Self {
        "0x3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29"
            .parse::<Id>()
            .unwrap()
    }

    /// Maximum id derived from the all-ones private key (test/invalid edge case).
    pub fn max_private_key_id() -> Self {
        "0x76a1592044a6e4f511265bca73a604d90b0529d1df602be30a19a9257660d1f5"
            .parse::<Id>()
            .unwrap()
    }

    /// Space-separated binary representation.
    pub fn to_binary_string_with_spaces(&self) -> String {
        let mut output = String::with_capacity(Self::BITS + Self::BYTES);
        for (index, byte) in self.0.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push_str(&format!("{byte:08b}"));
        }
        output
    }
}

// Create Id from a slice to the byte buffer
impl TryFrom<&[u8]> for Id {
    type Error = Error;
    fn try_from(input: &[u8]) -> Result<Self> {
        Id::try_from_bytes(input)
    }
}

// Create Id from a string (base58 or hex)
impl TryFrom<&str> for Id {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        if let Some(value) = value.strip_prefix(DID_PREFIX) {
            return Self::try_from_base58(value);
        } else if value.starts_with("0x") || value.starts_with("0X") {
            return Self::try_from_hexstr(value);
        } else {
            Self::try_from_base58(value)
        }
    }
}

impl str::FromStr for Id {
    type Err = Error;
    fn from_str(str: &str) -> Result<Self> {
        TryFrom::try_from(str)
    }
}

// Create Id from signature public key.
impl From<&signature::PublicKey> for Id {
    fn from(pk: &signature::PublicKey) -> Self {
        let mut bytes = [0u8; Id::BYTES];
        bytes.copy_from_slice(pk.as_bytes());
        Id(bytes)
    }
}

impl ops::Deref for Id {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for Id {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl fmt::Binary for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_binary_string())
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, se: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match se.is_human_readable() {
            true => se.serialize_str(&self.to_base58()),
            false => se.serialize_bytes(&self.0),
        }
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(de: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdVisitor;
        impl<'de> Visitor<'de> for IdVisitor {
            type Value = Id;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a base58 string or 32 byte array")
            }

            fn visit_str<E>(self, v: &str) -> StdResult<Self::Value, E>
            where
                E: de::Error,
            {
                TryFrom::try_from(v).map_err(|e| de::Error::custom(e))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> StdResult<Self::Value, E>
            where
                E: de::Error,
            {
                TryFrom::try_from(v).map_err(|e| de::Error::custom(e))
            }
        }

        match de.is_human_readable() {
            true => de.deserialize_str(IdVisitor),
            false => de.deserialize_bytes(IdVisitor),
        }
    }
}

#[cfg(test)]
#[path = "unitests/test_id.rs"]
mod test_id;
