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
    where S: Serializer,
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
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where D: Deserializer<'de>,
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
            where D: Deserializer<'de>,
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
                formatter.write_str("a StoreValueRequest struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>,
            {
                let mut token: Option<i32> = None;
                let mut expected_seq: Option<i32> = None;
                let mut pk: Option<Id> = None;
                let mut rec: Option<Id> = None;
                let mut nonce: Option<Vec<u8>> = None;
                let mut seq: Option<i32> = None;
                let mut sig: Option<Vec<u8>> = None;
                let mut data: Option<Vec<u8>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Token => {
                            if token.is_some() {
                                return Err(de::Error::duplicate_field("tok"));
                            } else {
                                token = Some(map.next_value()?);
                            }
                        }
                        Field::Cas => {
                            if expected_seq.is_some() {
                                return Err(de::Error::duplicate_field("cas"));
                            } else {
                                expected_seq = Some(map.next_value()?);
                            }
                        }
                        Field::PublicKey => {
                            if pk.is_some() {
                                return Err(de::Error::duplicate_field("k"));
                            } else {
                                pk = Some(map.next_value()?);
                            }
                        }
                        Field::Recipient => {
                            if rec.is_some() {
                                return Err(de::Error::duplicate_field("rec"));
                            } else {
                                rec = Some(map.next_value()?);
                            }
                        }
                        Field::Nonce => {
                            if nonce.is_some() {
                                return Err(de::Error::duplicate_field("n"));
                            } else {
                                nonce = Some(map.next_value()?);
                            }
                        }
                        Field::Seq => {
                            if seq.is_some() {
                                return Err(de::Error::duplicate_field("seq"));
                            } else {
                                seq = Some(map.next_value()?);
                            }
                        }
                        Field::Signature => {
                            if sig.is_some() {
                                return Err(de::Error::duplicate_field("sig"));
                            } else {
                                sig = Some(map.next_value()?);
                            }
                        }
                        Field::Data => {
                            if data.is_some() {
                                return Err(de::Error::duplicate_field("v"));
                            } else {
                                data = Some(map.next_value()?);
                            }
                        }
                        Field::Ignore => {
                            let _ = map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                let expected_seq = expected_seq.unwrap_or(-1);
                if expected_seq < -1 {
                    return Err(de::Error::custom("expected_seq must be larger than or equal to -1"));
                }
                let seq = seq.unwrap_or_default();
                if seq < 0 {
                    return Err(de::Error::custom("sequence number must be larger than or equal to 0"));
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
                    token.ok_or_else(|| de::Error::missing_field("tok"))?,
                    expected_seq
                ))
            }
        }

        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for StoreValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tok:{},v:[{}]", self.token, self.value)?;
        if self.expected_seq >= 0 {
            write!(f, ",cas:{}", self.expected_seq)?;
        }
        Ok(())
    }
}
