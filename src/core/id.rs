use std::cmp::Ordering;
use std::str::FromStr;
use std::ops::Deref;
use std::fmt;
use std::result::Result as SResult;
use bs58;
use serde::{
    Serialize,Deserialize,
    ser::Serializer,
    de::{self, Deserializer, Visitor}
};

use crate::{
    randomize_bytes,
    cryptobox,
    signature,
    Error,
    Result,
    errors::ArgumentError,
};

pub const DID_PREFIX: &str = "did:boson:";

#[derive(Debug, Copy, Clone, Default, PartialOrd, PartialEq, Ord, Eq, Hash)]
pub struct Id(
    [u8; Id::BYTES]
);

impl Id {
    pub const BYTES: usize = 32;
    pub const BITS:  usize = 256;

    pub const MAX_ID: Id = Id::max();
    pub const MIN_ID: Id = Id::min();

    pub fn random() -> Self {
        let mut bytes = [0u8; Id::BYTES];
        randomize_bytes(&mut bytes);
        Id(bytes)
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

    pub fn try_from_hexstr(input: &str) -> Result<Self> {
        let Some(input) = input.strip_prefix("0x") else {
            return Err(ArgumentError::new("Hex format strings must have a '0x' prefix.".into()));
        };

        let mut bytes = [0u8; Id::BYTES];
        hex::decode_to_slice(input, &mut bytes[..]).map_err(|e|
            ArgumentError::new(format!("Invalid hex format string: {e}"))
        )?;
        Ok(Id(bytes))
    }

    pub fn try_from_base58(input: &str) -> Result<Self> {
        if input.starts_with("0x") {
            return Err(ArgumentError::new("Base58 format strings must not have a '0x' prefix.".into()));
        }

        let mut bytes = [0u8; Id::BYTES];
        bs58::decode(input)
            .with_alphabet(bs58::Alphabet::DEFAULT)
            .onto(&mut bytes[..])
            .map_err(|e| {
                println!(">>> e: {}, input:{}", e, input);
                ArgumentError::new(format!("Invalid base58 format string: {e}"))
        })?;
        Ok(Id(bytes))
    }

    //  Creates an id with the specified bit set to 1.
    pub fn try_from_bit_at(index: usize) -> Result<Self> {
        if index >= Id::BITS {
            return Err(ArgumentError::new(format!(
                "Index {} is out of bounds for ID with {} bits",
                index, Id::BITS
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

    pub fn to_binary_string(&self) -> String {
        let mut out = String::with_capacity(Id::BYTES * 8);
        for b in &self.0 {
            out.push_str(&format!("{:08b}", b));
        }
        out
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
    pub(crate) fn update(&mut self, cb: impl Fn(&mut [u8])) {
        cb(self.0.as_mut_slice());
    }

    /*
    Order::Less     : means a is closer to the target than b.
    Order::Greater  : means a is farther from the target than b.
    Order::Equal    : means a and b are equidistant from the target.
     */
    pub(crate) fn three_way_compare(&self, a: &Self, b: &Self) -> Ordering {
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

    pub(crate) fn bits_equal(a: &Id, b: &Id, depth: i32) -> bool {
        if depth == -1 {
            return true;
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
    fn try_from(str: &str) -> Result<Self> {
        match str.starts_with("0x") {
            true => Self::try_from_hexstr(str),
            false => Self::try_from_base58(str)
        }
    }
}

impl FromStr for Id {
    type Err = Error;
    fn from_str(str: &str) -> Result<Self> {
        Self::try_from(str)
    }
}

// Create Id from signature public key.
impl From<&signature::PublicKey> for Id {
    fn from(pk: &signature::PublicKey) -> Self {
        Id(pk.0)
    }
}

impl Deref for Id {
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
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer,
    {
        match se.is_human_readable() {
            true => se.serialize_str(&self.to_base58()),
            false => se.serialize_bytes(&self.0),
        }
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdVisitor;
        impl<'de> Visitor<'de> for IdVisitor {
            type Value = Id;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a base58 string or 32 bytes")
            }

            fn visit_str<E>(self, value: &str) -> SResult<Self::Value, E>
            where E: de::Error,
            {
                Id::try_from(value).map_err(|e|
                    de::Error::custom(format!("Invalid ID string: {e}"))
                )
            }

            fn visit_bytes<E>(self, v: &[u8]) -> SResult<Self::Value, E>
            where E: de::Error,
            {
                Id::try_from_bytes(v).map_err(|e|
                    de::Error::custom(format!("Invalid ID bytes: {e}"))
                )
            }
        }

        match de.is_human_readable() {
            true => de.deserialize_str(IdVisitor),
            false => de.deserialize_bytes(IdVisitor),
        }
    }
}
