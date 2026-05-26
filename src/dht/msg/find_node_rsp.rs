use std::fmt;
use std::result::Result as SResult;
use serde_cbor::value::to_value;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{Serializer, SerializeMap },
};

use crate::{
    NodeInfo,
    dht::msg::lookup_rsp::{
        LookupResponse,
        Data as LookupData
    }
};
pub(crate) struct FindNodeResponse {
    data: LookupData,
}

impl FindNodeResponse {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>,
        token: i32
    ) -> Self {
        Self {
            data: LookupData::new(nodes4, nodes6, token)
        }
    }
}

impl LookupResponse for FindNodeResponse {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

impl Serialize for FindNodeResponse {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer
    {
        let mut s = se.serialize_map(None)?;
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
        s.serialize_entry("tok", &self.token())?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for FindNodeResponse {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where D: Deserializer<'de>,
    {
        enum Field {
            Nodes4,         // "n4" - Vec<NodeInfo>
            Nodes6,         // "n6" - Vec<NodeInfo>
            Token,          // "tok" - i32,
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
                    _       => Ok(Field::Ignore),
                }
            }
        }

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = FindNodeResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A FindNodeResponse struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>,
            {
                let mut nodes4: Option<Vec<NodeInfo>> = None;
                let mut nodes6: Option<Vec<NodeInfo>> = None;
                let mut token: Option<i32> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Nodes4 => {
                            if nodes4.is_some() {
                                return Err(serde::de::Error::duplicate_field("n4"));
                            } else {
                                nodes4 = Some(map.next_value()?);
                            }
                        }
                        Field::Nodes6 => {
                            if nodes6.is_some() {
                                return Err(serde::de::Error::duplicate_field("n6"));
                            } else {
                                nodes6 = Some(map.next_value()?);
                            }
                        }
                        Field::Token => {
                            if token.is_some() {
                                return Err(serde::de::Error::duplicate_field("tok"));
                            } else {
                                token = Some(map.next_value()?);
                            }
                        }
                        Field::Ignore => {
                            let _ = map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                Ok(FindNodeResponse::new(
                    nodes4,
                    nodes6,
                    token.ok_or_else(|| de::Error::missing_field("tok"))?
                ))
            }
        }
        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for FindNodeResponse {
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

        if self.token() != 0 {
            if has_written_section {
                write!(f, ",")?;
            }
            write!(f, "tok:{}", self.token())?;
        }
        Ok(())
    }
}
