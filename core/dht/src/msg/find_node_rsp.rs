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

use crate::msg::lookup_rsp::{Data as LookupData, LookupResponse};
use boson_api::{
    utils::codec,
    NodeInfo
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(into = "FindNodeResponseRaw", try_from = "FindNodeResponseRaw")]
pub(crate) struct FindNodeResponse {
    data: LookupData,
}

impl FindNodeResponse {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>,
        token: i32,
    ) -> Self {
        Self {
            data: LookupData::new(nodes4, nodes6, token),
        }
    }
}

impl LookupResponse for FindNodeResponse {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FindNodeResponseRaw {
    #[serde(
        rename = "n4",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    nodes4: Option<Vec<NodeInfo>>,

    #[serde(
        rename = "n6",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    nodes6: Option<Vec<NodeInfo>>,

    #[serde(
        rename = "tok",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    token: i32,
}

impl Into<FindNodeResponseRaw> for FindNodeResponse {
    fn into(self) -> FindNodeResponseRaw {
        FindNodeResponseRaw {
            nodes4: self.nodes4().map(|v| v.to_vec()),
            nodes6: self.nodes6().map(|v| v.to_vec()),
            token: self.token(),
        }
    }
}

impl From<FindNodeResponseRaw> for FindNodeResponse {
    fn from(s: FindNodeResponseRaw) -> Self {
        Self::new(s.nodes4, s.nodes6, s.token)
    }
}

impl fmt::Display for FindNodeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

#[cfg(test)]
#[path = "unitests/test_find_node_rsp.rs"]
mod test_find_node_rsp;
