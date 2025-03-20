use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use core::result;
use bs58::decode;
use hex::FromHexError;
use ciborium::value::Value;

use serde::{Serialize, Deserialize};
use serde_with::serde_as;

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

#[serde_as]
#[derive(Default, Serialize, Deserialize, Clone, PartialOrd, PartialEq, Ord, Eq, Debug, Hash)]
pub struct Id(
    #[serde_as(as = "serde_with::Bytes")] // Serialize as bytes
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
        let mut bytes = [0u8; ID_BYTES];
        hex::decode_to_slice(input, &mut bytes[..])
            .map_err(|e| match e {
                FromHexError::InvalidHexCharacter { c, index } => {
                    Error::Argument(format!("Invalid hex character {} at position {}", c, index))
                },
                FromHexError::OddLength => {
                    Error::Argument(format!("Odd hex string length {}", input.len()))
                },
                FromHexError::InvalidStringLength => {
                    Error::Argument(format!("Invalid hex string length"))
                }
            })?;
        Ok(Id(bytes))
    }

    pub fn try_from_base58(input: &str) -> Result<Self> {
        let mut bytes = [0u8; ID_BYTES];
        bs58::decode(input)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .onto(&mut bytes[..])
            .map_err(|e| match e {
                decode::Error::BufferTooSmall => {
                    Error::Argument(format!("Invalid base58 string length"))
                },
                decode::Error::InvalidCharacter { character, index } => {
                    Error::Argument(format!("Invalid base58 character {} at {}", character, index))
                },
                _ => {
                    Error::Argument(format!("Invalid base58 with unknown error"))
                }
            })?;
        Ok(Id(bytes))
    }

    pub const fn min() -> Self {
        Id([0x0; ID_BYTES])
    }

    pub const fn max() -> Self {
        Id([0xFF; ID_BYTES])
    }

    pub fn to_hexstr(&self) -> String {
        hex::encode(&self.0)
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(self.0)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .into_string()
    }

    pub fn to_signature_key(&self) -> signature::PublicKey {
        signature::PublicKey::try_from(self.as_bytes()).unwrap()
    }

    pub fn to_encryption_key(&self) -> cryptobox::PublicKey {
        cryptobox::PublicKey::try_from(&self.to_signature_key()).unwrap()
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

    pub(crate) fn to_cbor(&self) -> Value {
        Value::Bytes(self.0.to_vec())
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

impl FromStr for Id {
    type Err = Error;
    fn from_str(base58: &str) -> result::Result<Self, Self::Err> {
        Self::try_from_base58(base58)
    }
}

// Create Id from a base58 string
impl TryFrom<&str> for Id {
    type Error = Error;
    fn try_from(base58: &str) -> Result<Self> {
        Self::try_from_base58(base58)
    }
}

// Create Id from signature public key.
impl From<signature::PublicKey> for Id {
    fn from(pk: signature::PublicKey) -> Self {
        assert_eq!(ID_BYTES, pk.as_bytes().len());
        assert_eq!(ID_BYTES, signature::PublicKey::BYTES);

        Id(pk.0)
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

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "inspect")]
        write!(f,
            "0x{}/{}",
            self.to_hexstr(),
            self.to_base58()
        )?;

        #[cfg(not(feature = "inspect"))]
        write!(f, "{}", self.to_base58())?;

        Ok(())
    }
}

pub fn distance(a: &Id, b: &Id) -> Id {
    a.distance(b)
}
