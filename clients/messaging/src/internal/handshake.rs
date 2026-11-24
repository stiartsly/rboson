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

use serde::{Deserialize, Serialize};

use boson_api::{
    utils,
    Id,
    errors::{Result},
};
use crate::errors::EncodingError;

/// Friend-request handshake payload.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "y", content = "b")]
pub(crate) enum HandshakeType {
    #[serde(rename = "fr")]
    FriendRequest(Option<String>),
    #[serde(rename = "fra")]
    FriendRequestAccept(#[serde(with="utils::bytes")] Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Handshake {
    #[serde(rename = "t")]
    pub(crate) timestamp: i64,

    #[serde(flatten)]
    pub(crate) body: HandshakeType,

    #[serde(skip)]
    pub(crate) from: Option<Id>,
}

impl Handshake {
    pub(crate) fn friend_request(hello: Option<String>, timestamp: i64) -> Self {
        Self {
            timestamp: timestamp,
            body: HandshakeType::FriendRequest(hello),
            from: None,
        }
    }

    pub(crate) fn friend_request_accept(session_key: Vec<u8>, timestamp: i64) -> Self {
        Self {
            timestamp,
            body: HandshakeType::FriendRequestAccept(session_key),
            from: None,
        }
    }

    pub(crate) fn set_from(&mut self, from: Id) {
        self.from = Some(from);
    }

    pub(crate) fn serialize(&self) -> Vec<u8> {
        serde_cbor::to_vec(self).expect("Serializing handshake failed")
    }

    pub(crate) fn deserialize(payload: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(payload).map_err(|e|
            EncodingError::new(&format!("Deserialize handshake error: {e}")).into()
        )
    }
}

#[cfg(test)]
#[path = "../unitests/test_handshake.rs"]
mod test_handshake;
