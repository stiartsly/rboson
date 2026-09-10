use crate::dht::msg::lookup_rsp::{Data, LookupResponse};
use crate::{
    errors::{Error, ProtocolError, Result},
    utils, NodeInfo, PeerInfo,
};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;
use std::result::Result as StdResult;

#[derive(Clone, Serialize, Deserialize)]
#[serde(into = "SerdeFindPeerResponse", try_from = "SerdeFindPeerResponse")]
pub(crate) struct FindPeerResponse {
    data: Data,
    peers: Option<Vec<PeerInfo>>,
}

impl FindPeerResponse {
    pub(crate) fn with_nodes(nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>) -> Self {
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindPeerResponse {
    #[serde(rename = "n4", skip_serializing_if = "utils::is_default")]
    nodes4: Option<Vec<NodeInfo>>,
    #[serde(rename = "n6", skip_serializing_if = "utils::is_default")]
    nodes6: Option<Vec<NodeInfo>>,
    #[serde(rename = "tok")]
    token: i32,
    #[serde(
        rename = "p",
        default,
        serialize_with = "serialize_peers",
        deserialize_with = "deserialize_peers",
        skip_serializing_if = "utils::is_default"
    )]
    peers: Option<Vec<PeerInfo>>,
}

fn serialize_peers<S>(peers: &Option<Vec<PeerInfo>>, serializer: S) -> StdResult<S::Ok, S::Error>
where
    S: Serializer,
{
    match peers {
        Some(peers) if serializer.is_human_readable() => peers.serialize(serializer),
        Some(peers) => {
            let packed = serde_cbor::to_vec(peers).map_err(serde::ser::Error::custom)?;
            serializer.serialize_bytes(&packed)
        }
        _ => serializer.serialize_none(),
    }
}

fn deserialize_peers<'de, D>(deserializer: D) -> StdResult<Option<Vec<PeerInfo>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct PeersVisitor;

    impl<'de> Visitor<'de> for PeersVisitor {
        type Value = Vec<PeerInfo>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a CBOR byte string containing peers or a peer sequence")
        }

        fn visit_bytes<E>(self, packed: &[u8]) -> StdResult<Self::Value, E>
        where
            E: de::Error,
        {
            serde_cbor::from_slice(packed)
                .map_err(|e| E::custom(format!("decoding packed peers failed: {e}")))
        }

        fn visit_seq<A>(self, mut sequence: A) -> StdResult<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut peers = Vec::new();
            while let Some(peer) = sequence.next_element()? {
                peers.push(peer);
            }
            Ok(peers)
        }
    }

    struct OptionalPeersVisitor;

    impl<'de> Visitor<'de> for OptionalPeersVisitor {
        type Value = Option<Vec<PeerInfo>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an optional packed peer list")
        }

        fn visit_none<E>(self) -> StdResult<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> StdResult<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(PeersVisitor).map(Some)
        }
    }

    deserializer.deserialize_option(OptionalPeersVisitor)
}

impl Into<SerdeFindPeerResponse> for FindPeerResponse {
    fn into(self) -> SerdeFindPeerResponse {
        SerdeFindPeerResponse {
            nodes4: self.nodes4().map(|v| v.to_vec()),
            nodes6: self.nodes6().map(|v| v.to_vec()),
            token: self.token(),
            peers: self.peers().map(|v| v.to_vec()),
        }
    }
}

impl TryFrom<SerdeFindPeerResponse> for FindPeerResponse {
    type Error = Error;

    fn try_from(s: SerdeFindPeerResponse) -> Result<Self> {
        if s.peers.is_none() && s.nodes4.is_none() && s.nodes6.is_none() {
            return Err(ProtocolError::new(
                "either \"n4\", \"n6\" or \"p\" must be present",
            ));
        }

        if s.peers.is_some() && (s.nodes4.is_some() || s.nodes6.is_some()) {
            return Err(ProtocolError::new(
                "\"p\" cannot be combined with \"n4\" or \"n6\"",
            ));
        }

        Ok(match s.peers {
            Some(peers) => FindPeerResponse::with_peers(peers),
            _ => FindPeerResponse::with_nodes(s.nodes4, s.nodes6),
        })
    }
}

impl fmt::Display for FindPeerResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
