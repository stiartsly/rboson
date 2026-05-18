use std::fmt;
use std::result::Result as SResult;
use serde::{
    Serialize, Deserialize, Serializer, Deserializer,
    de::{self, Visitor, MapAccess, IgnoredAny},
    ser::SerializeMap,
};

use crate::{
    Id,
    Value,
    cryptobox::Nonce
};

pub(crate) struct StoreValueRequest {
    token: i32,
    expected_seq: i32,
    value: Value,
}

impl StoreValueRequest {
    pub(crate) fn new(
        value: Value,
        token: i32,
        expected_seq: i32
    ) -> Self {
        Self {
            token,
            expected_seq,
            value
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}

impl Serialize for StoreValueRequest {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = &self.value;
        let mut map = se.serialize_map(None)?;

        map.serialize_entry("tok", &self.token)?;
        if self.expected_seq >= 0 {
            map.serialize_entry("cas", &self.expected_seq)?;
        }
        if value.sequence_number() >= 0 {
            map.serialize_entry("seq", &value.sequence_number())?;
        }
        if let Some(pk) = value.public_key() {
            map.serialize_entry("k", &pk)?;
        }
        if let Some(rec) = value.recipient() {
            map.serialize_entry("rec", &rec)?;
        }
        if let Some(nonce) = value.nonce() {
            map.serialize_entry("n", nonce.as_bytes())?;
        }
        if let Some(sig) = value.signature() {
            map.serialize_entry("sig", &sig)?;
        }
        map.serialize_entry("v", value.data())?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for StoreValueRequest {
    fn deserialize<D>(de: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Token,          // "tok"
            Cas,            // "cas"
            PublicKey,      // "k"
            Recipient,      // "rec"
            Nonce,          // "n"
            Seq,            // "seq"
            Signature,      // "sig"
            Data,           // "v"
            Ignore
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "tok"   => Ok(Field::Token),
                    "cas"   => Ok(Field::Cas),
                    "k"     => Ok(Field::PublicKey),
                    "rec"   => Ok(Field::Recipient),
                    "n"     => Ok(Field::Nonce),
                    "seq"   => Ok(Field::Seq),
                    "sig"   => Ok(Field::Signature),
                    "v"     => Ok(Field::Data),
                    _       => Ok(Field::Ignore),
                }
            }
        }

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = StoreValueRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("StoreValueRequest")
            }

            fn visit_map<V>(self, mut map: V) -> std::result::Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut token: i32 = 0;
                let mut expected_seq: i32 = -1;
                let mut pk: Option<Id> = None;
                let mut rec: Option<Id> = None;
                let mut nonce: Option<Vec<u8>> = None;
                let mut seq: i32 = 0;
                let mut sig: Option<Vec<u8>> = None;
                let mut data: Option<Vec<u8>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Token        => token = map.next_value()?,
                        Field::Cas          => expected_seq = map.next_value()?,
                        Field::PublicKey    => pk = Some(map.next_value()?),
                        Field::Recipient    => rec = Some(map.next_value()?),
                        Field::Nonce        => nonce = Some(map.next_value()?),
                        Field::Seq          => seq = map.next_value()?,
                        Field::Signature    => sig = Some(map.next_value()?),
                        Field::Data         => data = Some(map.next_value()?),
                        Field::Ignore       => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                let Some(data) = data else {
                    return Err(de::Error::missing_field("v"));
                };

                if data.is_empty() {
                    return Err(de::Error::custom("data field \"v\" cannot be empty"));
                }


                let nonce = if let Some(nonce) = nonce.as_ref() {
                    Nonce::try_from(nonce.as_slice())
                        .map_err(|_| de::Error::custom("invalid nonce length"))?
                    .into()
                } else {
                    None
                };

                let value = Value::packed(pk, rec, nonce, sig, data, seq);
                Ok(StoreValueRequest::new(
                    value,
                    token,
                    expected_seq
                ))
            }
        }

        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for StoreValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tok:{},cas:{},val:[{}]", self.token, self.expected_seq,
            match self.value.public_key() {
                Some(pk) => pk.to_string(),
                None => String::from("unknown")
            }
        )
    }
}
