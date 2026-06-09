use std::fmt;
use serde::{Deserialize, Serialize};
use crate::{
    NodeInfo,
    PeerInfo,
    errors::{Error, Result, ProtocolError},
    dht::msg::lookup_rsp::{
        LookupResponse,
        Data
    }
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeFindPeerResponse", try_from = "SerdeFindPeerResponse")]
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindPeerResponse {
    #[serde(rename = "n4", skip_serializing_if = "crate::is_default")]
    nodes4: Option<Vec<NodeInfo>>,
    #[serde(rename = "n6", skip_serializing_if = "crate::is_default")]
    nodes6: Option<Vec<NodeInfo>>,
    #[serde(rename = "tok")]
    token: i32,
    #[serde(rename = "p", skip_serializing_if = "crate::is_default")]
    peers: Option<Vec<PeerInfo>>,
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
        if s.peers.is_none() &&
            s.nodes4.is_none() &&
            s.nodes6.is_none() {
            return Err(ProtocolError::new("either \"n4\", \"n6\" or \"p\" must be present"));
        }

        if s.peers.is_some() && (
            s.nodes4.is_some() ||
            s.nodes6.is_some()
        ) {
            return Err(ProtocolError::new("\"p\" cannot be combined with \"n4\" or \"n6\""));
        }

        Ok(match s.peers {
            Some(peers) => FindPeerResponse::with_peers(peers),
            None => FindPeerResponse::with_nodes(s.nodes4, s.nodes6)
        })
    }
}

impl fmt::Display for FindPeerResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
