use std::fmt;
use std::result::Result as SResult;
use serde_cbor::value::to_value;
use serde::{
    Deserialize, Serialize,
    de::{Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{Serializer, SerializeMap },
};

use crate::NodeInfo;
use super::lookup_rsp::{
    LookupResponse,
    Data as LookupData
};
pub(crate) struct FindNodeResponse {
    data   : LookupData,
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

    fn data_mut(&mut self) -> &mut LookupData {
        &mut self.data
    }
}

impl Serialize for FindNodeResponse {
    fn serialize<S>(&self, serializer: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = serializer.serialize_map(None)?;
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
    fn deserialize<D>(deserializer: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Nodes4,         // "n4" - Vec<NodeInfo>
            Nodes6,         // "n6" - Vec<NodeInfo>
            Token,          // "tok" - i32,
            Ignore
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(deserializer)?;
                match key.as_str() {
                    "n4"    => Ok(Field::Nodes4),
                    "n6"    => Ok(Field::Nodes6),
                    "tok"   => Ok(Field::Token),
                    _       => Ok(Field::Ignore), // Ignore unknown fields
                }
            }
        }

        struct FieldVisiter;

        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = FindNodeResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("invalid FindNodeResponse")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut nodes4: Option<Vec<NodeInfo>> = None;
                let mut nodes6: Option<Vec<NodeInfo>> = None;
                let mut token: i32 = 0;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Nodes4   => nodes4 = Some(map.next_value()?),
                        Field::Nodes6   => nodes6 = Some(map.next_value()?),
                        Field::Token    => token = map.next_value()?,
                        Field::Ignore   => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                Ok(FindNodeResponse::new(nodes4, nodes6, token))
            }
        }
        deserializer.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for FindNodeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(nodes4) = self.nodes4() {
            let mut first = true;
            if !nodes4.is_empty() {
                write!(f, "n4:")?;
                for item in nodes4.iter() {
                    if !first {
                        first = false;
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
            }
        }

        if let Some(nodes6) = self.nodes6() {
            let mut first = true;
            if !nodes6.is_empty() {
                write!(f, "n6:")?;
                for item in nodes6.iter() {
                    if !first {
                        first = false;
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
            }
        }

        if self.token() != 0 {
            write!(f, ",tok:{}", self.token())?;
        }
        Ok(())
    }
}
