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
    contact::ContactType,
    internal::PhotonContact,
    ContactHandler,
};

fn new_contact(kind: u8, revision: i32) -> PhotonContact {
    let contact_type = match kind {
        0 => ContactType::Auto,
        1 => ContactType::Friend,
        2 => ContactType::Channel,
        _ => panic!("Unknown contact type"),
    };
    let contact = PhotonContact::new(Id::random(), contact_type, 100)
        .edit()
        .set_name(Some("Alice".into()))
        .set_remark(Some("friend".into()))
        .set_tags(Some("test".into()))
        .set_muted(true)
        .set_blocked(true)
        .set_updated_at(200)
        .set_revision(revision - 1)
        .build();
    PhotonContact::from_contact(&contact)
}

#[test]
fn test_serde() {
    for kind in 0..=2 {
        let contact = new_contact(kind, 7);
        let encoded = serde_cbor::to_vec(&contact).unwrap();

        let decoded = serde_cbor::from_slice::<PhotonContact>(&encoded).unwrap();
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
fn test_contact_editor() {
    let contact = new_contact(1, 0);
    let editor = contact.edit();
    let editor = editor.set_remark(Some("updated".into()));
    let editor = editor.set_tags(None);
    let editor = editor.set_muted(false);
    let editor = editor.set_blocked(false);
    let edited = editor.build();

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
