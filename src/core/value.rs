use std::fmt;
use sha2::{Digest, Sha256};

use crate::unwrap;
use super::{
    cryptobox,
    signature,
    Id,
    signature::{
        KeyPair,
        PrivateKey
    },
    cryptobox::Nonce,
    Error, Result
};

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

#[derive(Clone)]
pub struct ValueBuilder<'a> {
    data: &'a [u8],
}

#[derive(Clone)]
pub struct SignedBuilder<'a> {
    keypair: Option<&'a KeyPair>,
    nonce: Option<&'a Nonce>,

    data: &'a [u8],
    seq: i32,
}

#[derive(Clone)]
pub struct EncryptedBuilder<'a> {
    keypair: Option<&'a KeyPair>,
    nonce: Option<&'a Nonce>,

    rec: &'a Id,
    data: &'a [u8],
    seq: i32,
}

impl<'a> ValueBuilder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        assert!(!data.is_empty());
        Self { data }
    }

    pub fn build(&self) -> Result<Value> {
        if self.data.len() == 0 {
            return Err(Error::Argument(format!("Value data cannot be empty")));
        }
        Ok(Value::new(self))
    }
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
        if self.data.len() == 0 {
            return Err(Error::Argument(format!("Value data cannot be empty")));
        }
        Ok(Value::signed(self))
    }
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
        if self.data.len() == 0 {
            return Err(Error::Argument(format!("Value data cannot be empty")));
        }
        Ok(Value::encrypted(self))
    }
}

pub(crate) struct PackBuilder {
    pk: Option<Id>,
    sk: Option<signature::PrivateKey>,
    rec: Option<Id>,
    nonce: Option<cryptobox::Nonce>,
    sig: Option<Vec<u8>>,
    data: Vec<u8>,
    seq: i32,
}

impl PackBuilder {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self {
            pk:     None,
            sk:     None,
            rec:    None,
            nonce:  None,
            sig:    None,
            data:   data,
            seq: 0,
        }
    }

    pub(crate) fn with_pk(mut self, pk: Option<Id>) -> Self {
        self.pk = pk;
        self
    }

    pub(crate) fn with_sk(mut self, sk: Option<PrivateKey>) -> Self {
        self.sk = sk;
        self
    }

    pub(crate) fn with_rec(mut self, recipient: Option<Id>) -> Self {
        self.rec = recipient;
        self
    }

    pub(crate) fn with_nonce(mut self, nonce: Option<Nonce>) -> Self {
        self.nonce = nonce;
        self
    }

    pub(crate) fn with_sig(mut self, sig: Option<Vec<u8>>) -> Self {
        self.sig = sig;
        self
    }

    pub(crate) fn with_seq(mut self, seq: i32) -> Self {
        self.seq = seq;
        self
    }

    pub(crate) fn build(self) -> Value {
        assert!(self.data.len() > 0);
        Value::packed(self)
    }
}

impl Value {
    fn new(b: &ValueBuilder) -> Value {
        assert!(b.data.len() > 0);

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

    fn signed(b: &SignedBuilder) -> Value {
        assert!(b.data.len() > 0);

        let kp = match b.keypair.as_ref() {
            Some(v) => v,
            None => &KeyPair::random()
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
        value.sig = sig.ok();
        value
    }

    fn encrypted(b: &EncryptedBuilder) -> Value {
        assert!(b.data.len() > 0);

        let kp = match b.keypair.as_ref() {
            Some(v) => v,
            None => &KeyPair::random()
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
        ).unwrap();

        // encrypt data.
        value.data = cryptobox::encrypt_into(
            value.data.as_ref(),
            value.nonce.as_ref().unwrap(),
            &unwrap!(value.recipient).to_encryption_key(),
            &encryption_sk,
        ).ok().unwrap();

        // sign data
        let sig = signature::sign_into(
            value.serialize_signature_data().as_slice(),
            value.sk.as_ref().unwrap()
        );
        value.sig = sig.ok();
        value
    }

    fn packed(mut b: PackBuilder) -> Self {
        Value {
            pk: b.pk.take(),
            sk: b.sk.take(),
            recipient: b.rec.take(),
            nonce: b.nonce.take(),
            sig: b.sig.take(),
            data: std::mem::take(&mut b.data),
            seq: b.seq,
        }
    }

    pub fn id(&self) -> Id {
        let input = match self.pk.as_ref() {
            Some(pk) => pk.as_bytes(),
            None => self.data.as_slice()
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

impl Into<Id> for Value {
    fn into(self: Value) -> Id {
        self.id()
    }
}

pub fn value_id(value: &Value) -> Id {
    value.id()
}
