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

use boson_api::{
    errors::{Error, Result},
    utils::codec,
    Id,
    PeerInfo,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(
    into = "AnnouncePeerRequestRaw",
    try_from = "AnnouncePeerRequestRaw"
)]
pub(crate) struct AnnouncePeerRequest {
    token: i32,
    peer: PeerInfo,
    expected_seq: i32, // None means unset
}

impl AnnouncePeerRequest {
    pub(crate) fn new(peer: PeerInfo, token: i32, expected_seq: Option<i32>) -> Self {
        Self {
            token,
            peer,
            expected_seq: expected_seq.unwrap_or(-1),
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn peer(&self) -> &PeerInfo {
        &self.peer
    }
}

#[derive(Serialize, Deserialize)]
struct AnnouncePeerRequestRaw {
    #[serde(rename = "tok")]
    token: i32,

    #[serde(
        rename = "cas",
        with = "codec::expected_seq"
    )]
    expected_seq: i32,

    #[serde(rename = "k", with = "codec::id")]
    id: Id,

    #[serde(
        rename = "seq",
        default,
        with = "codec::num",
        skip_serializing_if = "codec::is_default"
    )]
    seq: i32,

    #[serde(
        rename = "o",
        default,
        with = "codec::id_opt",
        skip_serializing_if = "Option::is_none"
    )]
    node_id: Option<Id>,

    #[serde(
        rename = "os",
        default,
        with = "codec::bytes_opt",
        skip_serializing_if = "codec::is_default"
    )]
    node_sig: Option<Vec<u8>>,

    #[serde(
        rename = "sig",
        with = "codec::bytes"
    )]
    sig: Vec<u8>,

    #[serde(rename = "f")]
    fingerprint: u64,

    #[serde(rename = "e")]
    endpoint: String,

    #[serde(
        rename = "ex",
        default,
        with = "codec::bytes_opt",
        skip_serializing_if = "codec::is_default"
    )]
    extra: Option<Vec<u8>>,
}

impl Into<AnnouncePeerRequestRaw> for AnnouncePeerRequest {
    fn into(self) -> AnnouncePeerRequestRaw {
        let peer = self.peer;
        AnnouncePeerRequestRaw {
            token: self.token,
            expected_seq: self.expected_seq,
            id: peer.id().clone(),
            seq: peer.sequence_number(),
            node_id: if peer.is_authenticated() {
                peer.nodeid().cloned()
            } else {
                None
            },
            node_sig: if peer.is_authenticated() {
                peer.node_signature().map(|v| v.to_vec())
            } else {
                None
            },
            sig: peer.signature().to_vec(),
            fingerprint: peer.fingerprint(),
            endpoint: peer.endpoint().to_string(),
            extra: peer.extra_data().map(|v| v.to_vec()),
        }
    }
}

impl TryFrom<AnnouncePeerRequestRaw> for AnnouncePeerRequest {
    type Error = Error;
    fn try_from(s: AnnouncePeerRequestRaw) -> Result<Self> {
        let peer = PeerInfo::packed(
            s.id,
            s.seq,
            s.node_id,
            s.node_sig,
            s.sig,
            s.fingerprint,
            s.endpoint,
            s.extra,
        );
        Ok(AnnouncePeerRequest {
            token: s.token,
            peer,
            expected_seq: s.expected_seq,
        })
    }
}

impl fmt::Display for AnnouncePeerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

#[cfg(test)]
#[path = "unitests/test_announce_peer_req.rs"]
mod test_announce_peer_req;
