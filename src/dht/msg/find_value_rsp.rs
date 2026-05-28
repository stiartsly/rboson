use std::{
    fmt,
    result::Result as SResult
};
use serde_cbor::value::to_value;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::{
    Id,
    Value,
    NodeInfo,
    cryptobox::Nonce,
    dht::msg::lookup_rsp::{
        LookupResponse,
        Data
    }
};

pub(crate) struct FindValueResponse {
    data: Data,
    value: Option<Value>,
}

impl FindValueResponse {
    pub(crate) fn with_nodes(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        Self {
            data: Data::new(nodes4, nodes6, 0),
            value: None,
        }
    }

    pub(crate) fn with_value(value: Value) -> Self {
        Self {
            data: Data::new(None, None, 0),
            value: Some(value),
        }
    }

    pub(crate) fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

impl LookupResponse for FindValueResponse {
    fn data(&self) -> &Data {
        &self.data
    }
}

impl Serialize for FindValueResponse {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer,
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
            s.serialize_entry("v", value.data())?;
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
    where D: Deserializer<'de>,
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
            where D: Deserializer<'de>,
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

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = FindValueResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a FindValueResponse struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>,
            {
                let mut nodes4  : Option<Vec<NodeInfo>> = None;
                let mut nodes6  : Option<Vec<NodeInfo>> = None;
                let mut pk      : Option<Id> = None;
                let mut rec     : Option<Id> = None;
                let mut nonce   : Option<Vec<u8>> = None;
                let mut sig     : Option<Vec<u8>> = None;
                let mut seq     : Option<i32> = None;
                let mut data    : Option<Vec<u8>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Nodes4 => {
                            if nodes4.is_some() {
                                return Err(de::Error::duplicate_field("n4"));
                            } else {
                                nodes4 = Some(map.next_value()?);
                            }
                        }
                        Field::Nodes6 => {
                            if nodes6.is_some() {
                                return Err(de::Error::duplicate_field("n6"));
                            } else {
                                nodes6 = Some(map.next_value()?);
                            }
                        }
                        Field::Key => {
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
                        Field::Signature => {
                            if sig.is_some() {
                                return Err(de::Error::duplicate_field("sig"));
                            } else {
                                sig = Some(map.next_value()?);
                            }
                        }
                        Field::SequenceNumber => {
                            if seq.is_some() {
                                return Err(de::Error::duplicate_field("seq"));
                            } else {
                                seq = Some(map.next_value()?);
                            }
                        }
                        Field::Data => {
                            if data.is_some() {
                                return Err(de::Error::duplicate_field("v"));
                            } else {
                                data = Some(map.next_value()?);
                            }
                        }
                        Field::Token |
                        Field::Ignore => {
                            let _ = map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                if data.is_none() &&
                    nodes4.is_none() &&
                    nodes6.is_none() {
                    return Err(de::Error::custom("either \"n4\", \"n6\" or \"v\" must be present"));
                }
                if data.is_some() && (
                    nodes4.is_some() ||
                    nodes6.is_some()
                ) {
                    return Err(de::Error::custom("\"v\" cannot be combined with \"n4\" or \"n6\""));
                }

                if let Some(data) = data {
                    if data.is_empty() {
                        return Err(de::Error::custom("data field \"v\" cannot be empty"));
                    }

                    let expected_seq = seq.unwrap_or(-1);
                    if expected_seq < -1 {
                        return Err(de::Error::custom("sequence number must be larger than or equal to -1"));
                    }

                    let nonce = nonce.map(|v| {
                        Nonce::try_from(v.as_slice())
                            .map_err(|_| de::Error::custom("invalid nonce length"))
                    }).transpose()?;

                    let value = Value::packed(pk, rec, nonce, sig, data, expected_seq);
                    if !value.is_valid() {
                        return Err(de::Error::custom("invalid value"));
                    }
                    Ok(FindValueResponse::with_value(value))
                } else {
                    Ok(FindValueResponse::with_nodes(nodes4, nodes6))
                }
            }
        }
        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for FindValueResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut has_written_section = false;

        if let Some(nodes4) = self.nodes4() {
            if !nodes4.is_empty() {
                write!(f, "n4:")?;
                for (index, item) in nodes4.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
                has_written_section = true;
            }
        }
        if let Some(nodes6) = self.nodes6() {
            if !nodes6.is_empty() {
                if has_written_section {
                    write!(f, ",")?;
                }
                write!(f, "n6:")?;
                for (index, item) in nodes6.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
                has_written_section = true;
            }
        }

        if let Some(value) = self.value() {
            if has_written_section {
                write!(f, ",")?;
            }
            write!(f, "v:[{}]", value)?;
        }
        Ok(())
    }
}
