use std::fmt;
use std::result::Result as SResult;
use serde_cbor::value::to_value;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::{ NodeInfo, PeerInfo };
use super::lookup_rsp::{
    LookupResponse,
    Data
};

pub(crate) struct FindPeerResponse {
    pub(crate) data: Data,
    pub(crate) peers: Option<Vec<PeerInfo>>,
}

impl FindPeerResponse {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        Self {
            data: Data::new(nodes4, nodes6, 0),
            peers: None,
        }
    }

    pub(crate) fn from(peers: Vec<PeerInfo>) -> Self {
        Self {
            data: Data::new(None, None, 0),
            peers: Some(peers),
        }
    }

    pub(crate) fn has_peers(&self) -> bool {
        self.peers.as_ref().map_or(false, |p| !p.is_empty())
    }

    pub(crate) fn peers(&self) -> Option<&[PeerInfo]> {
        self.peers.as_deref()
    }
}

impl LookupResponse for FindPeerResponse {
    fn data(&self) -> &Data {
        &self.data
    }

    fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }
}

impl Serialize for FindPeerResponse {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = se.serialize_map(None)?;
        if let Some(peers) = self.peers.as_ref() {
            let value = to_value(&peers).map_err(|_| serde::ser::Error::custom(
                "Failed to convert peers to CBOR Value"
            ))?;
            s.serialize_entry("p", &value)?;
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

impl<'de> Deserialize<'de> for FindPeerResponse {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Nodes4,         // "n4"
            Nodes6,         // "n6"
            Token,          // "tok"
            Peers,          // "p"
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
                    "p"     => Ok(Field::Peers),
                    "tok"   => Ok(Field::Token),
                    _       => Ok(Field::Ignore),
                }
            }
        }

        struct FieldVisiter;
        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = FindPeerResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("FindPeerResponse")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut nodes4 = None;
                let mut nodes6 = None;
                let mut peers = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Nodes4 => nodes4 = map.next_value()?,
                        Field::Nodes6 => nodes6 = map.next_value()?,
                        Field::Peers => peers = map.next_value()?,
                        Field::Token |
                        Field::Ignore => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                if nodes4.is_none() && nodes6.is_none() && peers.is_none() {
                    return Err(de::Error::custom("either \"n4\", \"n6\" or \"p\" must be present"));
                }

                if let Some(peers) = peers {
                    Ok(FindPeerResponse::from(peers))
                } else {
                    Ok(FindPeerResponse::new(nodes4, nodes6))
                }
            }
        }
        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for FindPeerResponse {
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

        let mut first = true;
        if let Some(peers) = self.peers.as_ref() {
            if peers.is_empty() {
                return Ok(());
            }

            write!(f, ",p:")?;
            for item in peers.iter() {
                if !first {
                    first = false;
                    write!(f, ",")?;
                }
                write!(f, "[{}]", item)?;
            }
        }
        Ok(())
    }
}
