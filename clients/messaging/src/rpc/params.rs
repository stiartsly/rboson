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

use serde::{Serialize, Serializer};

use boson_api::Id;

use crate::{channel::Role, internal::codec};

/// An `Id` that goes on the wire as a byte string, as the service expects.
pub(super) struct IdBytes<'a>(pub(super) &'a Id);

impl Serialize for IdBytes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0.as_bytes())
    }
}

pub(super) mod id_list {
    use super::*;

    pub(in crate::rpc) fn serialize<S: Serializer>(
        ids: &[Id],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(ids.iter().map(IdBytes))
    }
}

/// Parameters of `ChannelMembersRoleUpdate`.
#[derive(Debug, Serialize)]
pub(crate) struct ChannelMembersRole {
    #[serde(rename = "ids", serialize_with = "id_list::serialize")]
    members: Vec<Id>,

    #[serde(rename = "r", with = "codec::role")]
    role: Role,
}

impl ChannelMembersRole {
    pub(crate) fn new(members: Vec<Id>, role: Role) -> Self {
        Self { members, role }
    }
}
