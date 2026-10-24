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

use crate::{
    msg::lookup_req::{
        Data as LookupData, LookupRequest, WANT4_MASK, WANT6_MASK, WANT_TOKEN_MASK,
    },
};
use boson_api::{
    utils,
    Id,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(into = "SerdeFindNodeRequest", from = "SerdeFindNodeRequest")]
pub(crate) struct FindNodeRequest {
    data: LookupData,
}

impl FindNodeRequest {
    pub(crate) fn new(target: Id, want4: bool, want6: bool, want_token: bool) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, want_token),
        }
    }
}

impl LookupRequest for FindNodeRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindNodeRequest {
    #[serde(
        rename = "t",
        serialize_with = "utils::codec::id::serialize",
        deserialize_with = "utils::codec::id::deserialize"
    )]
    target: Id,
    #[serde(rename = "w")]
    want: i32,
}

impl Into<SerdeFindNodeRequest> for FindNodeRequest {
    fn into(self) -> SerdeFindNodeRequest {
        SerdeFindNodeRequest {
            target: self.target().clone(),
            want: self.want(),
        }
    }
}

impl From<SerdeFindNodeRequest> for FindNodeRequest {
    fn from(s: SerdeFindNodeRequest) -> Self {
        FindNodeRequest::new(
            s.target,
            s.want & WANT4_MASK != 0,
            s.want & WANT6_MASK != 0,
            s.want & WANT_TOKEN_MASK != 0,
        )
    }
}

impl fmt::Display for FindNodeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
