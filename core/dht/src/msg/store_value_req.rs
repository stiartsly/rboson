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
    cryptobox::Nonce,
    errors::{Error, ProtocolError, Result},
    utils::codec,
    Id,
    Value,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(
    into = "StoreValueRequestRaw",
    try_from = "StoreValueRequestRaw"
)]
pub(crate) struct StoreValueRequest {
    token: i32,
    expected_seq: i32,
    value: Value,
}

impl StoreValueRequest {
    pub(crate) fn new(value: Value, token: i32, expected_seq: i32) -> Self {
        Self {
            token,
            expected_seq,
            value,
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Serialize, Deserialize)]
struct StoreValueRequestRaw {
    #[serde(rename = "tok")]
    token: i32,

    #[serde(
        rename = "cas",
        default = "default_expected_seq",
        with = "codec::expected_seq",
    )]
    expected_seq: i32,

    #[serde(
        rename = "seq",
        default,
        with = "codec::num",
    )]
    seq: i32,

    #[serde(
        rename = "k",
        default,
        with = "codec::id_opt",
        skip_serializing_if = "Option::is_none"
    )]
    public_key: Option<Id>,

    #[serde(
        rename = "rec",
        default,
        with = "codec::id_opt",
        skip_serializing_if = "Option::is_none"
    )]
    recipient: Option<Id>,

    #[serde(
        rename = "n",
        default,
        with = "codec::bytes_opt",
        skip_serializing_if = "codec::is_default"
    )]
    nonce: Option<Vec<u8>>,

    #[serde(
        rename = "sig",
        default,
        with = "codec::bytes_opt",
        skip_serializing_if = "codec::is_default"
    )]
    signature: Option<Vec<u8>>,

    #[serde(
        rename = "v",
        with = "codec::bytes",
        skip_serializing_if = "codec::is_default"
    )]
    data: Vec<u8>,
}

fn default_expected_seq() -> i32 {
    -1
}

impl Into<StoreValueRequestRaw> for StoreValueRequest {
    fn into(self) -> StoreValueRequestRaw {
        let value = self.value;
        StoreValueRequestRaw {
            token: self.token,
            expected_seq: self.expected_seq,
            seq: value.sequence_number(),
            public_key: value.public_key().cloned(),
            recipient: value.recipient().cloned(),
            nonce: value.nonce().map(|n| n.as_bytes().to_vec()),
            signature: value.signature().map(|v| v.to_vec()),
            data: value.data().to_vec(),
        }
    }
}

impl TryFrom<StoreValueRequestRaw> for StoreValueRequest {
    type Error = Error;

    fn try_from(s: StoreValueRequestRaw) -> Result<Self> {
        if s.data.is_empty() {
            return Err(ProtocolError::new("data field \"v\" cannot be empty"));
        }

        let nonce = s
            .nonce
            .map(|v| {
                Nonce::try_from(v.as_slice())
                    .map_err(|_| ProtocolError::new("invalid nonce length"))
            })
            .transpose()?;

        let value = Value::packed(s.public_key, s.recipient, nonce, s.signature, s.data, s.seq);

        if !value.is_valid() {
            return Err(ProtocolError::new("The value is invalid"));
        }

        Ok(StoreValueRequest::new(value, s.token, s.expected_seq))
    }
}

impl fmt::Display for StoreValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

#[cfg(test)]
#[path = "unitests/test_store_value_req.rs"]
mod test_store_value_req;
