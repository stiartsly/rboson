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

use serde::Serialize;

use boson_api::{utils, Id};

use crate::{channel::Role, internal::codec};

#[derive(Debug, Serialize)]
pub(crate) struct ChannelMembersRole {
    #[serde(rename = "ids", serialize_with = "utils::ids::serialize")]
    members: Vec<Id>,

    #[serde(rename = "r", with = "codec::role")]
    role: Role,
}

impl ChannelMembersRole {
    pub(crate) fn new(members: Vec<Id>, role: Role) -> Self {
        Self { members, role }
    }
}

#[cfg(test)]
mod tests {
    use serde_cbor::Value;

    use super::*;

    #[test]
    fn channel_members_role_serializes_ids_for_text_and_cbor() {
        let id = Id::random();
        let value = ChannelMembersRole::new(vec![id], Role::Member);

        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(json["ids"][0], id.to_base58());
        assert_eq!(json["r"], 2);

        let cbor = serde_cbor::to_vec(&value).unwrap();
        let Value::Map(map) = serde_cbor::from_slice(&cbor).unwrap() else {
            panic!("channel members role must be a map");
        };
        assert_eq!(
            map.get(&Value::Text("ids".into())),
            Some(&Value::Array(vec![Value::Bytes(id.as_bytes().to_vec())]))
        );
    }
}
