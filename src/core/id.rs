use std::cmp::Ordering;
use std::fmt;
use core::result;
use ciborium::value::Value;
use bs58;
use serde_with::serde_as;
use serde::{
    Serialize,
    Serializer,
    Deserialize,
    Deserializer
};

use crate::{
    randomize_bytes,
    cryptobox,
    signature,
    Error,
    error::Result
};

pub const ID_BYTES: usize = 32;
pub const ID_BITS:  usize = 256;

pub const MIN_ID: Id = Id::min();
pub const MAX_ID: Id = Id::max();

pub const DID_PREFIX: &str = "did:boson:";

#[serde_as]
#[derive(Debug, Clone, Default, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize, Hash)]
pub struct Id(
    #[serde(with = "bytes_as_base58")]
    [u8; ID_BYTES]
);

impl Id {
    pub fn random() -> Self {
        let mut bytes = [0u8; ID_BYTES];
        randomize_bytes(&mut bytes);
        Id(bytes)
    }

    pub fn from_bytes(input: [u8; ID_BYTES]) -> Self {
        Id(input)
    }

    pub(crate) fn from_cbor(input: &Value) -> Option<Self> {
        let bytes = input.as_bytes()?;
        Some(Id(bytes.as_slice().try_into().unwrap()))
    }

    pub fn try_from_hexstr(input: &str) -> Result<Self> {
        let Some(input) = input.strip_prefix("0x") else {
            return Err(Error::Argument("Hex format strings must have a '0x' prefix.".into()));
        };

        let mut bytes = [0u8; ID_BYTES];
        hex::decode_to_slice(input, &mut bytes[..]).map_err(|e|
            Error::Argument(format!("Invalid hex format string: {e}"))
        )?;
        Ok(Id(bytes))
    }

    pub fn try_from_base58(input: &str) -> Result<Self> {
        if input.starts_with("0x") {
            return Err(Error::Argument("Base58 format strings must not have a '0x' prefix.".into()));
        }

        let mut bytes = [0u8; ID_BYTES];
        bs58::decode(input)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .onto(&mut bytes[..])
            .map_err(|e|
                Error::Argument(format!("Invalid base58 format string: {e}"))
            )?;
        Ok(Id(bytes))
    }

    //  Creates an id with the specified bit set to 1.
    pub fn try_from_bit_at(index: usize) -> Result<Self> {
        if index >= ID_BITS {
            return Err(Error::Argument(format!(
                "Index {} is out of bounds for ID with {} bits",
                index, ID_BITS
            )));
        }

        let mut bytes = [0u8; ID_BYTES];
        let byte_index = index / 8;
        let bit_index = index % 8;

        bytes[byte_index] |= 1 << (7 - bit_index);
        Ok(Id(bytes))
    }

    pub const fn min() -> Self {
        Id([0x0; ID_BYTES])
    }

    pub const fn max() -> Self {
        Id([0xFF; ID_BYTES])
    }

    pub const fn zero() -> Self {
        Id([0u8; ID_BYTES])
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

    pub fn to_binary_string(&self) -> String {
        unimplemented!()
    }

    pub fn to_abbr_str(&self) -> String {
        self.to_abbr_base58()
    }

    pub fn to_signature_key(&self) -> signature::PublicKey {
        signature::PublicKey::try_from(self.as_bytes()).unwrap()
    }

    pub fn to_encryption_key(&self) -> cryptobox::PublicKey {
        cryptobox::PublicKey::try_from(&self.to_signature_key()).unwrap()
    }

    pub(crate) fn to_cbor(&self) -> Value {
        Value::Bytes(self.0.to_vec())
    }

    pub fn distance(&self, other: &Id) -> Id {
        let mut bytes = [0u8; ID_BYTES];
        for i in 0..ID_BYTES {
            bytes[i] = self.0[i] ^ other.0[i];
        }
        Id(bytes)
    }

    pub const fn size(&self) -> usize {
        ID_BYTES
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub(crate) fn update(&mut self, cb: impl Fn(&mut [u8])) {
        cb(self.0.as_mut_slice());
    }

    pub(crate) fn three_way_compare(&self, a: &Self, b: &Self) -> Ordering {
        let mut mmi = ID_BYTES;
        for i in 0..ID_BYTES {
            if a.0[i] != b.0[i] {
                mmi = i;
                break;
            }
        }
        if mmi == ID_BYTES {
            return Ordering::Equal;
        }

        let ua = a.0[mmi] ^ self.0[mmi];
        let ub = b.0[mmi] ^ self.0[mmi];

        ua.cmp(&ub)
    }

    pub fn distance_between(a: &Id, b: &Id) -> Id {
        a.distance(b)
    }
}

// Create Id from a slice to the byte buffer
impl TryFrom<&[u8]> for Id {
    type Error = Error;
    fn try_from(input: &[u8]) -> Result<Self> {
        if input.len() != ID_BYTES {
            return Err(Error::Argument(format!(
                "Invalid bytes length {} for ID, expected length {}",
                input.len(),
                ID_BYTES
            )));
        }
        Ok(Id(input.try_into().unwrap()))
    }
}

// Create Id from a string (base58 or hex)
impl TryFrom<&str> for Id {
    type Error = Error;
    fn try_from(str: &str) -> Result<Self> {
        match str.starts_with("0x") {
            true => Self::try_from_hexstr(str),
            false => Self::try_from_base58(str)
        }
    }
}

// Create Id from signature public key.
impl From<signature::PublicKey> for Id {
    fn from(pk: signature::PublicKey) -> Self {
        Id(pk.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

pub(crate) fn bits_equal(a: &Id, b: &Id, depth: i32) -> bool {
    if depth == -1 {
        return true;
    }

    let mut mmi = usize::MAX;
    for i in 0..ID_BYTES {
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

pub(crate) fn bits_copy(src: &Id, dst: &mut Id, depth: i32) {
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

mod bytes_as_base58 {
    use super::*;
    pub fn serialize<S>(bytes: &[u8], serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = bs58::encode(bytes).into_string();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> result::Result<[u8; ID_BYTES], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let vec = bs58::decode(&s)
            .into_vec()
            .map_err(serde::de::Error::custom)?;

        if vec.len() != ID_BYTES {
            return Err(serde::de::Error::custom(format!(
                "Invalid length: expected {}, got {}",
                ID_BYTES,
                vec.len()
            )));
        }
        let mut arr = [0u8; ID_BYTES];
        arr.copy_from_slice(&vec);
        Ok(arr)
    }
}
