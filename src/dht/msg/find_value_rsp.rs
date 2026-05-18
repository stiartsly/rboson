use std::fmt;
use std::result::Result as SResult;
use serde_cbor::value::to_value;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::{
    Id,
    NodeInfo,
    Value,
    cryptobox::Nonce
};

use super::lookup_rsp::{
    LookupResponse,
    Data
};

pub(crate) struct FindValueResponse {
    pub(crate) data: Data,
    pub(crate) value: Option<Value>,
}

impl FindValueResponse {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        Self {
            data: Data::new(nodes4, nodes6, 0),
            value: None,
        }
    }

    pub(crate) fn from(value: Value) -> Self {
        Self {
            data: Data::new(None, None, 0),
            value: Some(value),
        }
    }

    pub(crate) fn has_value(&self) -> bool {
        self.value.is_some()
    }

    pub(crate) fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

impl LookupResponse for FindValueResponse {
    fn data(&self) -> &Data {
        &self.data
    }

    fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }
}

impl Serialize for FindValueResponse {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = se.serialize_map(None)?;
        if let Some(value) = self.value.as_ref() {
            if let Some(pk) = value.public_key() {
                s.serialize_entry("k", pk)?;
            }
            if let Some(rec) = value.recipient() {
                s.serialize_entry("rec", rec)?;
            }
            if let Some(n) = value.nonce() {
                s.serialize_entry("n", n.as_ref())?;
            }
            if value.sequence_number() >= 0 {
                s.serialize_entry("seq", &value.sequence_number())?;
            }
            if let Some(sig) = value.signature() {
                s.serialize_entry("sig", sig)?;
            }
        } else {
            if let Some(ns4) = self.nodes4() {
                let value = to_value(&ns4).map_err(|_| serde::ser::Error::custom(
                    "Failed to convert nodes4 to CBOR Value"
                ))?;
                s.serialize_entry("n4", &value)?;
            }
            if let Some(ns6) = self.nodes6() {
                let value = to_value(&ns6).map_err(|_| serde::ser::Error::custom(
                    "Failed to convert nodes6 to CBOR Value"
                ))?;
                s.serialize_entry("n6", &value)?;
            }
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for FindValueResponse {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Nodes4,         // "n4"
            Nodes6,         // "n6"
            Token,          // "tok"
            Key,            // "k"
            Recipient,      // "rec"
            Nonce,          // "n"
            Signature,      // "sig"
            SequenceNumber, // "seq"
            Data,           // "v"
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "n4"    => Ok(Field::Nodes4),
                    "n6"    => Ok(Field::Nodes6),
                    "tok"   => Ok(Field::Token),
                    "k"     => Ok(Field::Key),
                    "rec"   => Ok(Field::Recipient),
                    "n"     => Ok(Field::Nonce),
                    "sig"   => Ok(Field::Signature),
                    "seq"   => Ok(Field::SequenceNumber),
                    "v"     => Ok(Field::Data),
                    _       => Ok(Field::Ignore)
                }
            }
        }

        struct FieldVisiter;

        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = FindValueResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("FindValueResponse")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut nodes4 = None;
                let mut nodes6 = None;
                let mut pk: Option<Id> = None;
                let mut rec: Option<Id> = None;
                let mut nonce: Option<Vec<u8>> = None;
                let mut sig: Option<Vec<u8>> = None;
                let mut seq: i32 = -1;
                let mut data: Option<Vec<u8>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Nodes4 => nodes4 = Some(map.next_value()?),
                        Field::Nodes6 => nodes6 = Some(map.next_value()?),
                        Field::Key => pk = Some(map.next_value()?),
                        Field::Recipient => rec = Some(map.next_value()?),
                        Field::Nonce => nonce = Some(map.next_value()?),
                        Field::Signature => sig = Some(map.next_value()?),
                        Field::SequenceNumber => seq = map.next_value()?,
                        Field::Data => data = map.next_value()?,
                        Field::Token |
                        Field::Ignore => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                if nodes4.is_none() && nodes6.is_none() && data.is_none() {
                    return Err(de::Error::custom("either \"n4\", \"n6\" or \"v\" must be present"));
                }

                if let Some(data) = data {
                    if data.len() == 0 {
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
                    if !value.is_valid() {
                        return Err(de::Error::custom("invalid value in FindValueResponse"));
                    }
                    Ok(FindValueResponse::from(value))
                } else {
                    Ok(FindValueResponse::new(nodes4, nodes6))
                }
            }
        }
        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for FindValueResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
