// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::msg::lookup_rsp::{Data, LookupResponse};
use boson_api::{
    errors::{Error, ProtocolError, Result},
    utils,
    NodeInfo,
    PeerInfo,
};
use serde::{
    Deserialize, Serialize,
};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(into = "FindPeerResponseRaw", try_from = "FindPeerResponseRaw")]
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
struct FindPeerResponseRaw {
    #[serde(
        rename = "n4",
        skip_serializing_if = "utils::is_default"
    )]
    nodes4: Option<Vec<NodeInfo>>,

    #[serde(
        rename = "n6",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    nodes6: Option<Vec<NodeInfo>>,

    #[serde(
        rename = "tok",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    token: i32,

    #[serde(
        rename = "p",
        default,
        with = "utils::peers",
        skip_serializing_if = "utils::is_default"
    )]
    peers: Vec<PeerInfo>,
}

impl Into<FindPeerResponseRaw> for FindPeerResponse {
    fn into(self) -> FindPeerResponseRaw {
        FindPeerResponseRaw {
            nodes4: self.nodes4().map(|v| v.to_vec()),
            nodes6: self.nodes6().map(|v| v.to_vec()),
            token: self.token(),
            peers: self.peers().map(|v| v.to_vec()).unwrap_or(Vec::new()),
        }
    }
}

impl TryFrom<FindPeerResponseRaw> for FindPeerResponse {
    type Error = Error;

    fn try_from(s: FindPeerResponseRaw) -> Result<Self> {
        if s.peers.is_empty() && s.nodes4.is_none() && s.nodes6.is_none() {
            return Err(ProtocolError::new(
                "either \"n4\", \"n6\" or \"p\" must be present",
            ));
        }
        if !s.peers.is_empty() && (s.nodes4.is_some() || s.nodes6.is_some()) {
            return Err(ProtocolError::new(
                "\"p\" cannot be combined with \"n4\" or \"n6\"",
            ));
        }

        Ok(match s.peers.is_empty() {
            false => FindPeerResponse::with_peers(s.peers),
            true => FindPeerResponse::with_nodes(s.nodes4, s.nodes6),
        })
    }
}

impl fmt::Display for FindPeerResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

#[cfg(test)]
#[path = "unitests/test_find_peer_rsp.rs"]
mod test_find_peer_rsp;