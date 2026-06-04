use std::fmt;
use std::result::Result as SResult;
use sha2::{Digest, Sha256};
use serde::{
    Serialize, Deserialize,
    Serializer, Deserializer,
    ser::SerializeStruct,
    de::{self, Visitor, MapAccess}
};

use super::{
    Id,
    cryptobox,
    signature,
    signature::{KeyPair, PrivateKey},
    cryptobox::Nonce,
    Result,
    errors::ArgumentError
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

impl<'a> ValueBuilder<'a> {
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

#[derive(Clone)]
pub struct EncryptedBuilder<'a> {
    keypair: Option<&'a KeyPair>,
    nonce: Option<&'a Nonce>,

    rec: &'a Id,
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

impl Value {
    fn new(b: &ValueBuilder) -> Value {
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

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut len = 2; // seq + data
        if self.pk.is_some() { len += 1; }
        if self.recipient.is_some() { len += 1; }
        if self.nonce.is_some() { len += 1; }
        if self.sig.is_some() { len += 1; }

        let mut state = serializer.serialize_struct("Value", len)?;

        if let Some(pk) = &self.pk {
            state.serialize_field("k", pk)?;
        }
        if let Some(rec) = &self.recipient {
            state.serialize_field("rec", rec)?;
        }
        if let Some(n) = &self.nonce {
            state.serialize_field("n", n.as_ref())?;
        }
        if let Some(s) = &self.sig {
             state.serialize_field("s", s)?;
        }

        state.serialize_field("seq", &self.seq)?;
        state.serialize_field("v", &self.data)?;

        state.end()
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Key,            // "k"
            Recipient,      // "rec"
            Nonce,          // "n"
            Signature,      // "s"
            SequenceNumber, // "seq"
            Data,           // "v"
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(deserializer)?;
                match key.as_str() {
                    "k"     => Ok(Field::Key),
                    "rec"   => Ok(Field::Recipient),
                    "n"     => Ok(Field::Nonce),
                    "s"     => Ok(Field::Signature),
                    "seq"   => Ok(Field::SequenceNumber),
                    "v"     => Ok(Field::Data),
                    _ => Err(de::Error::unknown_field(&key, &["k", "rec", "n", "s", "seq", "v"])),
                }
            }
        }

        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Value")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut pk: Option<Id> = None;
                let mut recipient: Option<Id> = None;
                let mut raw_nonce: Option<Vec<u8>> = None;
                let mut sig: Option<Vec<u8>> = None;
                let mut seq: i32 = 0;
                let mut data: Option<Vec<u8>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Key              => pk = Some(map.next_value()?),
                        Field::Recipient        => recipient = Some(map.next_value()?),
                        Field::Nonce            => raw_nonce = Some(map.next_value()?),
                        Field::Signature        => sig = Some(map.next_value()?),
                        Field::SequenceNumber   => seq = map.next_value()?,
                        Field::Data             => data = Some(map.next_value()?),
                    }
                }

                let nonce = if let Some(raw_nonce) = raw_nonce.as_ref() {
                    if raw_nonce.len() != Nonce::BYTES {
                        return Err(de::Error::custom(
                            format!("Invalid nonce length: expected {} bytes, got {}", Nonce::BYTES, raw_nonce.len())));
                    }
                    Some(Nonce::try_from(raw_nonce.as_slice())
                        .map_err(|e| de::Error::custom(format!("Invalid nonce: {}", e)))?)
                } else {
                    None
                };

                let data = data.ok_or_else(|| de::Error::missing_field("v"))?;
                Ok(Value::packed(pk, recipient, nonce, sig, data, seq))
            }
        }
        deserializer.deserialize_map(ValueVisitor)
    }
}
