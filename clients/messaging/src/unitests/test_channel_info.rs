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

use std::collections::BTreeMap;
use serde_cbor::{
    value::from_value,
    Value
};

use boson_api::Id;
use crate::{
    channel::Permission,
    internal::ChannelInfo,
};

#[test]
fn test_serde() {
    let ids = |id: Id| id.as_bytes().to_vec();
    let map = [
        (Value::Text("id".into()), Value::Bytes(ids(Id::random()))),
        (Value::Text("o".into()),  Value::Bytes(ids(Id::random()))),
        (Value::Text("sid".into()), Value::Bytes(ids(Id::random()))),
        (Value::Text("p".into()), Value::Integer(0)),
    ].into_iter().collect::<BTreeMap<_, _>>();

    let origin = from_value::<ChannelInfo>(
        Value::Map(map.clone())
    ).unwrap();

    assert_eq!(origin.permission(), Permission::Public);
    assert_eq!(origin.session_key(), None);
    assert_eq!(origin.name(), None);
    assert_eq!(origin.notice(), None);
    assert_eq!(origin.announce(), false);
    assert_eq!(origin.created_at(), boson_api::as_ms!(std::time::UNIX_EPOCH) as u64);
    assert_eq!(origin.updated_at(), boson_api::as_ms!(std::time::UNIX_EPOCH) as u64);
    assert!(origin.members().is_empty());

    for field in ["id", "o", "sid", "p"] {
        let mut updated = map.clone();
        updated.remove(&Value::Text(field.into()));

        let rc =  from_value::<ChannelInfo>(Value::Map(updated));
        assert!(rc.is_err(),"{field}");
    }

    let mut updated = map.clone();
    updated.insert(Value::Text("sk".into()), Value::Null);

    let rc = serde_cbor::value::from_value(Value::Map(updated));
    let info: ChannelInfo = rc.unwrap();
    assert_eq!(info.session_key(), None);

    for permission in [-1, 4] {
        let mut updated = map.clone();
        updated.insert(Value::Text("p".into()), Value::Integer(permission));

        let info = from_value::<ChannelInfo>(Value::Map(updated));
        assert!(info.is_err());
    }

    let encoded = serde_cbor::to_vec(&origin).unwrap();
    let decoded: ChannelInfo = serde_cbor::from_slice(&encoded).unwrap();
    assert_eq!(decoded, origin);
}
