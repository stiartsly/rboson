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

use crate::msg::lookup_req::{Data as LookupData, LookupRequest, WANT4_MASK, WANT6_MASK};
use boson_api::{
    errors::{Error, Result},
    utils, Id,
};
use serde::de::{Deserializer, Error as DeError};
use serde::{Deserialize, Serialize};
use std::{fmt, result::Result as StdResult};

#[derive(Clone, Serialize, Deserialize)]
#[serde(into = "SerdeFindPeerRequest", try_from = "SerdeFindPeerRequest")]
pub(crate) struct FindPeerRequest {
    data: LookupData,
    expected_seq: i32,
    expected_count: i32,
}

impl FindPeerRequest {
    pub(crate) fn new(
        target: Id,
        want4: bool,
        want6: bool,
        expected_seq: i32,
        expected_count: i32,
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, false),
            expected_seq,
            expected_count,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn expected_count(&self) -> i32 {
        self.expected_count
    }
}

impl LookupRequest for FindPeerRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindPeerRequest {
    #[serde(
        rename = "t",
        serialize_with = "utils::codec::id::serialize",
        deserialize_with = "utils::codec::id::deserialize"
    )]
    target: Id,

    #[serde(rename = "w")]
    want: i32,

    #[serde(
        rename = "cas",
        default = "default_expected_seq",
        deserialize_with = "utils::codec::expected_seq::deserialize",
        skip_serializing_if = "is_default_expected_seq"
    )]
    expected_seq: i32,

    #[serde(
        rename = "e",
        deserialize_with = "deserialize_count",
        skip_serializing_if = "utils::is_default"
    )]
    expected_count: i32,
}

fn deserialize_count<'de, D>(de: D) -> StdResult<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let count = i32::deserialize(de)?;
    if count < 0 {
        return Err(DeError::custom("count must be larger than or equal to 0"));
    }
    Ok(count)
}

fn default_expected_seq() -> i32 {
    -1
}

fn is_default_expected_seq(value: &i32) -> bool {
    *value == -1
}

impl Into<SerdeFindPeerRequest> for FindPeerRequest {
    fn into(self) -> SerdeFindPeerRequest {
        SerdeFindPeerRequest {
            target: self.target().clone(),
            want: self.want(),
            expected_seq: self.expected_seq,
            expected_count: self.expected_count,
        }
    }
}

impl TryFrom<SerdeFindPeerRequest> for FindPeerRequest {
    type Error = Error;
    fn try_from(s: SerdeFindPeerRequest) -> Result<Self> {
        Ok(FindPeerRequest::new(
            s.target,
            s.want & WANT4_MASK != 0,
            s.want & WANT6_MASK != 0,
            s.expected_seq,
            s.expected_count,
        ))
    }
}

impl fmt::Display for FindPeerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
