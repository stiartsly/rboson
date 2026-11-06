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


    let id_vec = |id: Id| id.as_bytes().to_vec();
    let channel_info = [
        (Value::Text("id".into()), Value::Bytes(id_vec(Id::random()))),
        (Value::Text("o".into()),  Value::Bytes(id_vec(Id::random()))),
        (Value::Text("sid".into()), Value::Bytes(id_vec(Id::random()))),
        (Value::Text("p".into()), Value::Integer(0)),
    ].into_iter().collect::<std::collections::BTreeMap<_, _>>();

    let decoded = from_value::<ChannelInfo>(
        Value::Map(channel_info.clone())
    ).unwrap();

    assert_eq!(decoded.permission(), Permission::Public);
    assert_eq!(decoded.session_key(), None);
    assert_eq!(decoded.name(), None);
    assert_eq!(decoded.notice(), None);
    assert_eq!(decoded.announce(), false);
    assert_eq!(decoded.created_at(), std::time::UNIX_EPOCH);
    assert_eq!(decoded.updated_at(), std::time::UNIX_EPOCH);
    assert!(decoded.members().is_empty());

    for field in ["id", "o", "sid", "p"] {
        let mut wire = channel_info.clone();
        wire.remove(&Value::Text(field.into()));

        let rc =  from_value::<ChannelInfo>(Value::Map(wire));
        assert!(rc.is_err(),"{field}");
    }

    let mut wire = channel_info.clone();
    wire.insert(Value::Text("sk".into()), Value::Null);

    let rc = serde_cbor::value::from_value(Value::Map(wire));
    let info: ChannelInfo = rc.unwrap();
    assert_eq!(info.session_key(), None);

    for permission in [-1, 4] {
        let mut wire = channel_info.clone();
        wire.insert(Value::Text("p".into()), Value::Integer(permission));

        let info = from_value::<ChannelInfo>(Value::Map(wire));
        assert!(info.is_err());
    }
}
/*
#[test]
fn test_serde_metadata() {
    let channel_id = Id::random();
    let owner_id = Id::random();
    let session_id = Id::random();
    let mut info = ChannelInfo::new(
        channel_id,
        owner_id,
        session_id,
        Some(vec![1, 2, 3]),
        Permission::OwnerInvite,
        Some("channel".into()),
        Some("notice".into()),
        true,
        10,
        11,
        vec![ChannelMember::new(owner_id, Role::Owner, 10)],
    );

    let decoded: ChannelInfo = serde_cbor::value::from_value(serde_cbor::value::to_value(&info).unwrap()).unwrap();
    assert_eq!(decoded.channel_id(), &channel_id);
    assert_eq!(decoded.owner_id(), &owner_id);
    assert_eq!(decoded.session_id(), &session_id);
    assert_eq!(decoded.session_key(), Some([1, 2, 3].as_slice()));
    assert_eq!(decoded.permission(), Permission::OwnerInvite);
    assert_eq!(decoded.name(), Some("channel"));
    assert_eq!(decoded.notice(), Some("notice"));
    assert_eq!(decoded.announce(), true);
    assert_eq!(decoded.created_at(), std::time::UNIX_EPOCH + std::time::Duration::from_millis(10));
    assert_eq!(decoded.updated_at(), std::time::UNIX_EPOCH + std::time::Duration::from_millis(11));
    assert_eq!(decoded.members(), info.members());
    assert_eq!(decoded, info);

    use std::hash::{
        Hash,
        Hasher,
    };

    let mut original_hash = std::collections::hash_map::DefaultHasher::new();
    let mut decoded_hash = std::collections::hash_map::DefaultHasher::new();
    info.hash(&mut original_hash);
    decoded.hash(&mut decoded_hash);
    assert_eq!(original_hash.finish(), decoded_hash.finish());

    info.session_key = Some(Vec::new());
    info.name = Some(String::new());
    info.notice = Some(String::new());
    info.announce = false;
    info.members.clear();
    let Value::Map(wire) = serde_cbor::value::to_value(&info).unwrap() else {
        panic!("Channel info must serialize as a map");
    };
    for field in ["sk", "nt", "a", "m"] {
        assert!(!wire.contains_key(&Value::Text(field.into())), "{field}");
    }
    assert_eq!(wire.get(&Value::Text("n".into())), Some(&Value::Text(String::new())));
}
*/
