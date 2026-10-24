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

use crate::channel::Role;
use crate::internal::codec;
use boson_api::Id;

/// Role change request for one or more channel members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelMembersRole {
    #[serde(rename = "ids")]
    member_ids: Vec<Id>,
    #[serde(rename = "r", with = "codec::role")]
    role: Role,
}

impl ChannelMembersRole {
    #[cfg(test)]
    pub(crate) fn new(member_ids: Vec<Id>, role: Role) -> Self {
        Self { member_ids, role }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn serde_channel_members_role() {
        let dto = ChannelMembersRole::new(vec![Id::default()], Role::Moderator);
        let value = serde_cbor::value::to_value(&dto).unwrap();
        let Value::Map(fields) = &value else {
            panic!("channel member role must serialize as a map");
        };

        assert_eq!(
            fields.get(&Value::Text("r".to_string())),
            Some(&Value::Integer(1))
        );
        let decoded: ChannelMembersRole =
            serde_cbor::from_slice(&serde_cbor::to_vec(&dto).unwrap()).unwrap();
        assert_eq!(decoded, dto);
    }
}
