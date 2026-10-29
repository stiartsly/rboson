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


use boson_api::Id;
use crate::{
    internal::contact::ContactData,
    contact::Contact,
};
use serde_cbor::Value;

fn contact_data(kind: u8, revision: i32) -> ContactData {
    let mut data = ContactData::new(Id::random(), kind, 100);
    data.name = Some("Alice".into());
    data.remark = Some("friend".into());
    data.tags = Some("test".into());
    data.muted = true;
    data.blocked = true;
    data.updated_at = 200;
    data.revision = revision;
    data
}

#[test]
fn parses_contact() {
    for kind in 0..=2 {
        let contact_data = contact_data(kind, 7);
        let contact = Contact::try_from(contact_data.clone()).unwrap();
        let encoded = serde_cbor::to_vec(&contact).unwrap();
        assert_eq!(
            serde_cbor::from_slice::<Value>(&encoded).unwrap(),
            serde_cbor::value::to_value(&contact_data).unwrap(),
        );

        let decoded: Contact = serde_cbor::from_slice(&encoded).unwrap();
        assert_eq!(decoded.id(), contact.id());
        assert_eq!(decoded.contact_type(), contact.contact_type());
        assert_eq!(decoded.name(), contact.name());
        assert_eq!(decoded.remark(), contact.remark());
        assert_eq!(decoded.tags(), contact.tags());
        assert_eq!(decoded.is_muted(), contact.is_muted());
        assert_eq!(decoded.is_blocked(), contact.is_blocked());
        assert_eq!(decoded.created_at(), contact.created_at());
        assert_eq!(decoded.updated_at(), contact.updated_at());
        assert_eq!(decoded.revision(), contact.revision());
    }
}

#[test]
fn rejects_invalid_contact() {
    let mut invalids = vec![contact_data(3, 0), contact_data(1, -1)];
    let mut negative_timestamp = contact_data(1, 0);
    negative_timestamp.created_at = -1;
    invalids.push(negative_timestamp);
    let mut earlier_update = contact_data(1, 0);
    earlier_update.updated_at = earlier_update.created_at - 1;
    invalids.push(earlier_update);
    for data in invalids {
        assert!(Contact::try_from(data.clone()).is_err());
        let bytes = serde_cbor::to_vec(&data).unwrap();
        assert!(serde_cbor::from_slice::<Contact>(&bytes).is_err());
    }
}

#[test]
fn editor_preserves_contact() {
    let contact = Contact::try_from(contact_data(1, 0)).unwrap();
    let editor = Box::new(contact.edit()).remark(Some("updated".into()));
    let editor = Box::new(editor).tags(None);
    let editor = Box::new(editor).muted(false);
    let editor = Box::new(editor).blocked(false);
    let edited = Box::new(editor).build();

    assert_eq!(edited.id(), contact.id());
    assert_eq!(edited.remark(), Some("updated"));
    assert_eq!(edited.tags(), None);
    assert!(!edited.is_muted());
    assert!(!edited.is_blocked());
    assert_eq!(edited.revision(), 1);

    assert_eq!(contact.remark(), Some("friend"));
    assert_eq!(contact.tags(), Some("test"));
    assert!(contact.is_muted());
    assert!(contact.is_blocked());
    assert_eq!(contact.revision(), 0);
}
