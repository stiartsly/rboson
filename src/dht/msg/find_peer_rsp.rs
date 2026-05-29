use std::{
    fmt,
    result::Result as SResult
};
use serde_cbor::value::to_value;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{self, SerializeMap, Serializer}
};
use crate::{
    NodeInfo,
    PeerInfo,
    dht::msg::lookup_rsp::{
        LookupResponse,
        Data
    }
};

pub(crate) struct FindPeerResponse {
    data: Data,
    peers: Option<Vec<PeerInfo>>,
}

impl FindPeerResponse {
    pub(crate) fn with_nodes(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        Self {
            data: Data::new(nodes4, nodes6, 0),
            peers: None,
        }
    }

    pub(crate) fn with_peers(peers: Vec<PeerInfo>) -> Self {
        Self {
            data: Data::new(None, None, 0),
            peers: Some(peers),
        }
    }

    pub(crate) fn peers(&self) -> Option<&[PeerInfo]> {
        self.peers.as_deref()
    }
}

impl LookupResponse for FindPeerResponse {
    fn data(&self) -> &Data {
        &self.data
    }
}

impl Serialize for FindPeerResponse {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer,
    {
        let mut s = se.serialize_map(None)?;
        if let Some(peers) = self.peers.as_ref() {
            let value = to_value(&peers).map_err(|e| ser::Error::custom(
                format!("Convert peers to CBOR error: {}", e)
            ))?;
            s.serialize_entry("p", &value)?;
        }

        if let Some(ns) = self.nodes4() {
            let value = to_value(&ns).map_err(|e| ser::Error::custom(
                format!("Convert nodes4 to CBOR error: {}", e)
            ))?;
            s.serialize_entry("n4", &value)?;
        }
        if let Some(ns) = self.nodes6() {
            let value = to_value(&ns).map_err(|e| ser::Error::custom(
                format!("Convert nodes6 to CBOR error: {}", e)
            ))?;
            s.serialize_entry("n6", &value)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for FindPeerResponse {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where D: Deserializer<'de>,
    {
        enum Field {
            Nodes4,         // "n4"
            Nodes6,         // "n6"
            Token,          // "tok"
            Peers,          // "p"
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
                    "p"     => Ok(Field::Peers),
                    "tok"   => Ok(Field::Token),
                    _       => Ok(Field::Ignore),
                }
            }
        }

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = FindPeerResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a FindPeerResponse struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut nodes4: Option<Vec<NodeInfo>> = None;
                let mut nodes6: Option<Vec<NodeInfo>> = None;
                let mut peers : Option<Vec<PeerInfo>> = None;

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
                        Field::Peers => {
                            if peers.is_some() {
                                return Err(de::Error::duplicate_field("p"));
                            } else {
                                peers = Some(map.next_value()?);
                            }
                        }
                        Field::Token |
                        Field::Ignore => {
                            let _ = map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                if peers.is_none() &&
                    nodes4.is_none() &&
                    nodes6.is_none() {
                    return Err(de::Error::custom("either \"n4\", \"n6\" or \"p\" must be present"));
                }
                if peers.is_some() && (
                    nodes4.is_some() ||
                    nodes6.is_some()
                ) {
                    return Err(de::Error::custom("\"p\" cannot be combined with \"n4\" or \"n6\""));
                }

                Ok(match peers {
                    Some(peers) => FindPeerResponse::with_peers(peers),
                    None => FindPeerResponse::with_nodes(nodes4, nodes6)
                })
            }
        }
        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for FindPeerResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut has_sep = false;

        if let Some(nodes4) = self.nodes4() {
            if !nodes4.is_empty() {
                write!(f, "n4:")?;
                for (index, item) in nodes4.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
                has_sep = true;
            }
        }

        if let Some(nodes6) = self.nodes6() {
            if !nodes6.is_empty() {
                if has_sep {
                    write!(f, ",")?;
                }
                write!(f, "n6:")?;
                for (index, item) in nodes6.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
                has_sep = true;
            }
        }

        if let Some(peers) = self.peers.as_ref() {
            if peers.is_empty() {
                return Ok(());
            }

            if has_sep {
                write!(f, ",")?;
            }
            write!(f, "p:")?;
            for (index, item) in peers.iter().enumerate() {
                if index > 0 {
                    write!(f, ",")?;
                }
                write!(f, "[{}]", item)?;
            }
        }
        Ok(())
    }
}
