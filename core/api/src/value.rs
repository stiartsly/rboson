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
use sha2::{Digest, Sha256};
use std::{fmt, result::Result as StdResult};

use crate::{
    cryptobox,
    cryptobox::Nonce,
    errors::{ArgumentError, Error, StateError},
    signature,
    signature::{KeyPair, PrivateKey},
    CryptoIdentity, Id, Identity, Result,
};
use crate::utils::codec;

#[derive(Clone)]
pub struct ImmutableBuilder<'a> {
    data: &'a [u8],
}

impl<'a> ImmutableBuilder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn build(&self) -> Result<Value> {
        if self.data.is_empty() {
            return Err(ArgumentError::new("Value data cannot be empty"));
        }
        Ok(Value::new(self))
    }
}

#[derive(Clone)]
pub struct SignedBuilder<'a> {
    keypair: Option<&'a KeyPair>,
    nonce: Option<&'a Nonce>,

    data: &'a [u8],
    seq: i32,
}

impl<'a> SignedBuilder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            keypair: None,
            nonce: None,
            seq: 0,
        }
    }

    pub fn with_keypair(&mut self, keypair: &'a KeyPair) -> &mut Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn with_nonce(&mut self, nonce: &'a Nonce) -> &mut Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn with_sequence_number(&mut self, sequence_number: i32) -> &mut Self {
        self.seq = sequence_number;
        self
    }

    pub fn build(&self) -> Result<Value> {
        if self.data.is_empty() {
            return Err(ArgumentError::new("Value data cannot be empty"));
        }
        Value::signed(self)
    }
}

#[derive(Clone)]
pub struct EncryptedBuilder<'a> {
    keypair: Option<&'a KeyPair>,
    nonce: Option<&'a Nonce>,

    rec: &'a Id,
    data: &'a [u8],
    seq: i32,
}

impl<'a> EncryptedBuilder<'a> {
    pub fn new(data: &'a [u8], recipient: &'a Id) -> Self {
        Self {
            data: data,
            keypair: None,
            nonce: None,
            seq: 0,
            rec: recipient,
        }
    }

    pub fn with_keypair(&mut self, keypair: &'a KeyPair) -> &mut Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn with_nonce(&mut self, nonce: &'a Nonce) -> &mut Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn with_sequence_number(&mut self, sequence_number: i32) -> &mut Self {
        self.seq = sequence_number;
        self
    }

    pub fn build(&self) -> Result<Value> {
        if self.data.is_empty() {
            return Err(ArgumentError::new("Value data cannot be empty"));
        }
        Value::encrypted(self)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeValue", try_from = "SerdeValue")]
#[derive(Clone, Debug, Eq)]
pub struct Value {
    pk: Option<Id>,
    sk: Option<PrivateKey>,
    recipient: Option<Id>,
    nonce: Option<Nonce>,
    sig: Option<Vec<u8>>,
    data: Vec<u8>,
    seq: i32,
}

impl Value {
    pub const NONCE_BYTES: usize = Nonce::BYTES;
    pub const MAX_IMMUTABLE_DATA_BYTES: usize = 1024;
    pub const MAX_MUTABLE_DATA_BYTES: usize = 896;
    pub const MAX_ENCRYPTED_DATA_BYTES: usize = 832;

    fn new(b: &ImmutableBuilder) -> Value {
        assert!(!b.data.is_empty());

        Self {
            pk: None,
            sk: None,
            recipient: None,
            nonce: None,
            sig: None,
            data: b.data.to_vec(),
            seq: 0,
        }
    }

    fn signed(b: &SignedBuilder) -> Result<Value> {
        assert!(!b.data.is_empty());

        let kp = match b.keypair.as_ref() {
            Some(v) => v,
            _ => &KeyPair::random(),
        };
        let mut value = Value {
            pk: Some(Id::from(kp.public_key())),
            sk: Some(kp.to_private_key()),
            recipient: None,
            nonce: None,
            sig: None,
            data: b.data.to_vec(),
            seq: b.seq,
        };

        // sign data.
        let sig = signature::sign_into(
            value.serialize_signature_data().as_slice(),
            value.sk.as_ref().unwrap(),
        );

        match sig {
            Ok(s) => value.sig = Some(s),
            Err(e) => return Err(e.into()),
        }
        Ok(value)
    }

    fn encrypted(b: &EncryptedBuilder) -> Result<Value> {
        assert!(!b.data.is_empty());

        let kp = match b.keypair.as_ref() {
            Some(v) => v,
            _ => &KeyPair::random(),
        };

        let mut value = Value {
            pk: Some(Id::from(kp.public_key())),
            sk: Some(kp.to_private_key()),
            recipient: Some(b.rec.clone()),
            nonce: Some(b.nonce.map_or(Nonce::random(), |v| v.clone())),
            data: b.data.to_vec(),
            sig: None,
            seq: b.seq,
        };

        let encryption_sk = cryptobox::PrivateKey::try_from(value.sk.as_ref().unwrap())?;

        // encrypt data; the nonce is stored separately on the value.
        let nonce = value.nonce.as_ref().unwrap();
        let mut cipher = vec![0u8; value.data.len() + cryptobox::CryptoBox::MAC_BYTES];
        cryptobox::encrypt(
            value.data.as_ref(),
            &mut cipher,
            nonce,
            &value.recipient.as_ref().unwrap().to_encryption_key()?,
            &encryption_sk,
        )?;
        value.data = cipher;

        // sign data
        let sig = signature::sign_into(
            value.serialize_signature_data().as_slice(),
            value.sk.as_ref().unwrap(),
        )?;
        value.sig = Some(sig);
        Ok(value)
    }

    #[doc(hidden)]
    pub fn packed(
        pk: Option<Id>,
        recipient: Option<Id>,
        nonce: Option<Nonce>,
        sig: Option<Vec<u8>>,
        data: Vec<u8>,
        seq: i32,
    ) -> Self {
        Value {
            pk,
            sk: None,
            recipient,
            nonce,
            sig,
            data,
            seq,
        }
    }

    pub fn id(&self) -> Id {
        let input = match self.pk.as_ref() {
            Some(pk) => pk.as_bytes(),
            _ => self.data.as_slice(),
        };

        Id::try_from({
            let mut sha256 = Sha256::new();
            sha256.update(input);
            sha256.finalize().as_slice()
        })
        .unwrap()
    }

    pub const fn public_key(&self) -> Option<&Id> {
        self.pk.as_ref()
    }

    pub const fn recipient(&self) -> Option<&Id> {
        self.recipient.as_ref()
    }

    pub const fn has_private_key(&self) -> bool {
        self.sk.is_some()
    }

    pub const fn private_key(&self) -> Option<&signature::PrivateKey> {
        self.sk.as_ref()
    }

    pub const fn sequence_number(&self) -> i32 {
        self.seq
    }

    pub const fn nonce(&self) -> Option<&cryptobox::Nonce> {
        self.nonce.as_ref()
    }

    pub fn signature(&self) -> Option<&[u8]> {
        self.sig.as_ref().map(|s| s.as_slice())
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn size(&self) -> usize {
        self.data.len() + self.sig.as_ref().map_or(0, |s| s.len())
    }

    pub const fn is_encrypted(&self) -> bool {
        self.recipient.is_some()
    }

    pub const fn is_signed(&self) -> bool {
        self.sig.is_some()
    }

    pub const fn is_mutable(&self) -> bool {
        self.pk.is_some()
    }

    pub fn is_valid(&self) -> bool {
        if self.data.is_empty() || self.data.len() > self.max_data_bytes() {
            return false;
        }

        let Some(public_key) = self.pk.as_ref() else {
            return self.recipient.is_none()
                && self.nonce.is_none()
                && self.seq >= 0
                && self.sig.is_none()
                && self.id() == Self::immutable_id(&self.data);
        };

        let Some(signature) = self.sig.as_ref() else {
            return false;
        };
        if signature.len() != signature::Signature::BYTES || self.seq < 0 {
            return false;
        }
        match (self.recipient.is_some(), self.nonce.is_some()) {
            (true, true) | (false, false) => {}
            _ => return false,
        }
        if self.recipient.is_some() && self.data.len() < cryptobox::CryptoBox::MAC_BYTES {
            return false;
        }

        signature::verify(
            self.serialize_signature_data().as_slice(),
            signature,
            &public_key.to_signature_key(),
        )
        .unwrap_or(false)
    }

    fn immutable_id(data: &[u8]) -> Id {
        let mut sha256 = Sha256::new();
        sha256.update(data);
        Id::try_from_bytes(sha256.finalize().as_slice()).expect("SHA-256 is 32 bytes")
    }

    pub fn is_immutable(&self) -> bool {
        self.pk.is_none()
    }

    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    pub fn max_data_bytes(&self) -> usize {
        if self.is_encrypted() {
            Self::MAX_ENCRYPTED_DATA_BYTES
        } else if self.is_mutable() {
            Self::MAX_MUTABLE_DATA_BYTES
        } else {
            Self::MAX_IMMUTABLE_DATA_BYTES
        }
    }

    /// Decrypts an encrypted value for its intended recipient.
    pub fn decrypt_data(&self, recipient: &CryptoIdentity) -> Result<Vec<u8>> {
        let Some(pk) = self.pk.as_ref() else {
            return Err(StateError::new("Cannot decrypt: value is immutable"));
        };
        let Some(expected) = self.recipient.as_ref() else {
            return Err(StateError::new("Cannot decrypt: value is not encrypted"));
        };
        if recipient.id() != expected {
            return Err(ArgumentError::new(
                "Recipient identity does not match value recipient",
            ));
        }
        if !self.is_valid() {
            return Err(ArgumentError::new("Cannot decrypt: value failed validation"));
        }

        let nonce = self
            .nonce
            .as_ref()
            .ok_or_else(|| ArgumentError::new("Cannot decrypt: value has no nonce"))?;
        if self.data.len() < cryptobox::CryptoBox::MAC_BYTES {
            return Err(ArgumentError::new("Cannot decrypt: ciphertext is too short"));
        }
        let mut plain = vec![0u8; self.data.len() - cryptobox::CryptoBox::MAC_BYTES];
        cryptobox::decrypt(
            &self.data,
            &mut plain,
            nonce,
            &pk.to_encryption_key()?,
            recipient.encryption_key().private_key(),
        )?;
        Ok(plain)
    }

    pub(crate) fn serialize_signature_data(&self) -> Vec<u8> {
        let mut sha256 = Sha256::new();
        if let Some(pk) = self.pk.as_ref() {
            sha256.update(pk.as_bytes());

            if let Some(rec) = self.recipient.as_ref() {
                sha256.update(rec.as_bytes());
                if let Some(nonce) = self.nonce.as_ref() {
                    sha256.update(nonce.as_bytes());
                }
            }
            sha256.update(self.seq.to_le_bytes().as_ref());
        }
        sha256.update(self.data.as_slice());
        sha256.finalize().to_vec()
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.pk == other.pk
            && self.recipient == other.recipient
            && self.nonce == other.nonce
            && self.sig == other.sig
            && self.data == other.data
            && self.seq == other.seq
    }
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pk.hash(state);
        self.recipient.hash(state);
        self.nonce.as_ref().map(|nonce| nonce.as_bytes()).hash(state);
        self.sig.hash(state);
        self.data.hash(state);
        self.seq.hash(state);
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        write!(f, "id:{}", self.id())?;
        if self.is_mutable() {
            write!(
                f,
                ",publicKey:{}, nonce:{}",
                self.pk.as_ref().unwrap(),
                self.nonce
                    .as_ref()
                    .map(|nonce| nonce.to_string())
                    .unwrap_or_else(|| "null".to_string())
            )?;
        }
        if self.is_encrypted() {
            write!(f, ",recipient:{}", self.recipient.as_ref().unwrap())?;
        }
        if self.is_signed() {
            write!(f, ",sig:{}", hex::encode(self.sig.as_ref().unwrap()))?;
        }
        write!(
            f,
            ", seq:{}, data:{}",
            self.seq,
            hex::encode(self.data.as_slice())
        )?;
        write!(f, "}}")?;
        Ok(())
    }
}

pub fn value_id(value: &Value) -> Id {
    value.id()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeValue {
    #[serde(
        rename = "k",
        default,
        serialize_with = "codec::id_opt::serialize",
        deserialize_with = "codec::id_opt::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pk: Option<Id>,

    #[serde(
        rename = "rec",
        default,
        serialize_with = "codec::id_opt::serialize",
        deserialize_with = "codec::id_opt::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    recipient: Option<Id>,

    #[serde(
        rename = "n",
        default,
        serialize_with = "codec::nonce_opt::serialize",
        deserialize_with = "codec::nonce_opt::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    nonce: Option<Nonce>,

    #[serde(
        rename = "sig",
        default,
        serialize_with = "codec::bytes_opt::serialize",
        deserialize_with = "codec::bytes_opt::deserialize",
        skip_serializing_if = "codec::is_default"
    )]
    sig: Option<Vec<u8>>,

    #[serde(
        rename = "v",
        serialize_with = "codec::bytes::serialize",
        deserialize_with = "codec::bytes::deserialize"
    )]
    data: Vec<u8>,

    #[serde(
        rename = "seq",
        default,
        deserialize_with = "codec::seq::deserialize",
        skip_serializing_if = "crate::utils::is_default"
    )]
    seq: i32,
}

impl From<Value> for SerdeValue {
    fn from(value: Value) -> Self {
        Self {
            pk: value.pk,
            recipient: value.recipient,
            nonce: value.nonce,
            sig: value.sig,
            data: value.data,
            seq: value.seq,
        }
    }
}

impl TryFrom<SerdeValue> for Value {
    type Error = Error;

    fn try_from(v: SerdeValue) -> StdResult<Self, Self::Error> {
        if v.data.is_empty() {
            return Err(ArgumentError::new("value data cannot be empty"));
        }

        let mutable = v.pk.is_some();
        if mutable && v.sig.is_none() {
            return Err(ArgumentError::new("mutable value requires a signature"));
        }
        if v.recipient.is_some() && !mutable {
            return Err(ArgumentError::new("encrypted value requires a public key"));
        }
        if v.recipient.is_some() != v.nonce.is_some() {
            return Err(ArgumentError::new(
                "encrypted value requires a nonce, plain value must not carry one",
            ));
        }

        Ok(Value::packed(v.pk, v.recipient, v.nonce, v.sig, v.data, v.seq))
    }
}

#[cfg(test)]
#[path = "unitests/test_value.rs"]
mod test_value;
