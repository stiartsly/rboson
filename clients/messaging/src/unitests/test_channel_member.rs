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

use serde_cbor::Value;

use boson_api::Id;
use crate::channel::Role;
use super::ChannelMember;

#[test]
fn test_invalid_values() {
    for (role, joined) in [("Unknown", 12), ("Owner", -1)] {
        let wire = Value::Map([
            (Value::Text("id".into()), Value::Bytes(Id::random().as_bytes().to_vec())),
            (Value::Text("r".into()), Value::Text(role.into())),
            (Value::Text("j".into()), Value::Integer(joined.into())),
        ].into_iter().collect());

        let encoded = serde_cbor::to_vec(&wire).unwrap();
        let decoded = serde_cbor::from_slice::<ChannelMember>(&encoded);
        assert!(decoded.is_err());
    }
}

#[test]
fn test_serde() {
    let roles = [
        Role::Owner,
        Role::Moderator,
        Role::Member,
        Role::Banned
    ];

    for role in roles {
        let member = ChannelMember::new(Id::random(), role, 12);
        let encoded = serde_cbor::to_vec(&member).unwrap();
        let decoded = serde_cbor::from_slice::<Value>(&encoded).unwrap();

        let Value::Map(fields) = decoded else {
            panic!("Channel member must serialize as a map");
        };
        assert_eq!(
            fields.get(&Value::Text("r".into())),
            Some(&Value::Integer((role as i32).into()))
        );

        let decoded = serde_cbor::from_slice::<ChannelMember>(&encoded).unwrap();
        assert_eq!(decoded, member);
        assert_eq!(decoded.id(), member.id());
        assert_eq!(decoded.role(), role);
        assert_eq!(decoded.joined(), 12);
    }
}
