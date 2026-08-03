use std::{
    fmt,
    result::Result as SResult
};
use sha2::{Digest, Sha256};
use serde::{Serialize, Deserialize};

use super::{
    Id,
    cryptobox,
    signature,
    signature::{KeyPair, PrivateKey},
    cryptobox::Nonce,
    Result,
    errors::{Error, ArgumentError},
    utils
};

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
#[derive(Clone, Debug, PartialEq, Eq)]
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
            _ => &KeyPair::random()
        };
        let mut value = Value {
            pk: Some(Id::from(kp.public_key())),
            sk: Some(kp.to_private_key()),
            recipient: None,
            nonce: Some(b.nonce.map_or(Nonce::random(), |v|v.clone())),
            sig: None,
            data: b.data.to_vec(),
            seq: b.seq
        };

        // sign data.
        let sig = signature::sign_into(
            value.serialize_signature_data().as_slice(),
            value.sk.as_ref().unwrap()
        );

        match sig {
            Ok(s) => value.sig = Some(s),
            Err(e) => return Err(e.into())
        }
        Ok(value)
    }

    fn encrypted(b: &EncryptedBuilder) -> Result<Value> {
        assert!(!b.data.is_empty());

        let kp = match b.keypair.as_ref() {
            Some(v) => v,
            _ => &KeyPair::random()
        };

        let mut value = Value {
            pk: Some(Id::from(kp.public_key())),
            sk: Some(kp.to_private_key()),
            recipient: Some(b.rec.clone()),
            nonce: Some(b.nonce.map_or(Nonce::random(), |v|v.clone())),
            data: b.data.to_vec(),
            sig: None,
            seq: b.seq,
        };

        let encryption_sk = cryptobox::PrivateKey::try_from(
            value.sk.as_ref().unwrap()
        )?;

        // encrypt data.
        value.data = cryptobox::encrypt_into(
            value.data.as_ref(),
            value.nonce.as_ref().unwrap(),
            &value.recipient.as_ref().unwrap().to_encryption_key(),
            &encryption_sk,
        )?;

        // sign data
        let sig = signature::sign_into(
            value.serialize_signature_data().as_slice(),
            value.sk.as_ref().unwrap()
        )?;
        value.sig = Some(sig);
        Ok(value)
    }

    pub(crate) fn packed(
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
            _ => self.data.as_slice()
        };

        Id::try_from({
            let mut sha256 = Sha256::new();
            sha256.update(input);
            sha256.finalize().as_slice()
        }).unwrap()
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
        self.data.len() +
            self.sig.as_ref().map_or(0, |s|s.len())
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
        if self.data.is_empty() {
            return false;
        }
        if !self.is_mutable() {
            return true;
        }

        if self.pk.is_none() || self.sig.is_none() ||
            self.nonce.is_none() {
            return false;
        }

        signature::verify(
            self.serialize_signature_data().as_slice(),
            self.sig.as_ref().unwrap().as_slice(),
            &self.pk.as_ref().unwrap().to_signature_key(),
        ).is_ok()
    }

    pub(crate) fn serialize_signature_data(&self) -> Vec<u8> {
        let mut sha256 = Sha256::new();
        if let Some(pk) = self.pk.as_ref() {
            sha256.update(pk.as_bytes());

            if let Some(rec) = self.recipient.as_ref() {
                sha256.update(rec.as_bytes());
            }
            sha256.update(self.nonce.as_ref().unwrap().as_bytes());
            sha256.update(self.seq.to_le_bytes().as_ref());
        }
        sha256.update(self.data.as_slice());
        sha256.finalize().to_vec()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id:{}", self.id())?;
        if self.is_mutable() {
            write!(f,
                ",publicKey:{}, nonce:{}",
                self.pk.as_ref().unwrap(),
                self.nonce.as_ref().unwrap()
            )?;
        }
        if self.is_encrypted() {
            write!(f,
                ",recipient:{}",
                self.recipient.as_ref().unwrap()
            )?;
        }
        if self.is_signed() {
            write!(f,
                ",sig:{}",
                hex::encode(self.sig.as_ref().unwrap())
            )?;
        }
        write!(f,
            ", seq:{}, data:{}",
            self.seq,
            hex::encode(self.data.as_slice())
        )?;
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
        serialize_with = "utils::serialize_id_opt",
        deserialize_with = "utils::deserialize_id_opt",
        skip_serializing_if = "crate::is_default"
    )]
    pk: Option<Id>,

    #[serde(
        rename = "rec",
        default,
        serialize_with = "utils::serialize_id_opt",
        deserialize_with = "utils::deserialize_id_opt",
        skip_serializing_if = "crate::is_default"
    )]
    recipient: Option<Id>,

    #[serde(
        rename = "n",
        default,
        serialize_with = "utils::serialize_nonce_opt",
        deserialize_with = "utils::deserialize_nonce_opt",
        skip_serializing_if = "crate::is_default"
    )]
    nonce: Option<Nonce>,

    #[serde(
        rename = "s",
        default,
        serialize_with = "utils::serialize_sig_opt",
        deserialize_with = "utils::deserialize_sig_opt",
        skip_serializing_if = "crate::is_default"
    )]
    sig: Option<Vec<u8>>,

    #[serde(
        rename = "v",
        serialize_with = "utils::serialize_bytes",
        deserialize_with = "utils::deserialize_bytes"
    )]
    data: Vec<u8>,

    #[serde(
        rename = "seq",
        default = "utils::default_seq",
        serialize_with = "utils::serialize_seq",
        deserialize_with = "utils::deserialize_seq",
        skip_serializing_if = "utils::is_default_seq"
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

    fn try_from(v: SerdeValue) -> SResult<Self, Self::Error> {
        if v.data.is_empty() {
            return Err(ArgumentError::new("value data cannot be empty"));
        }

        let mutable = v.pk.is_some();
        if mutable && (v.sig.is_none() || v.nonce.is_none()) {
            return Err(ArgumentError::new(
                "mutable value requires both signature and nonce"
            ));
        }
        if v.recipient.is_some() && !mutable {
            return Err(ArgumentError::new(
                "encrypted value requires a public key"
            ));
        }

        let value = Value::packed(
            v.pk,
            v.recipient,
            v.nonce,
            v.sig,
            v.data,
            v.seq,
        );
        if !value.is_valid() {
            return Err(ArgumentError::new(
                "invalid value: signature verification failed"
            ));
        }
        Ok(value)
    }
}
