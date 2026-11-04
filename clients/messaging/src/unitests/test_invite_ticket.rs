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

use std::time::SystemTime;

use boson_api::{
    CryptoIdentity,
    Id,
};
use super::InviteTicket;

#[test]
fn test_serde() {
    let inviter = CryptoIdentity::new();
    let channel_id = Id::random();
    let session_id = Id::random();
    let invitee = Id::random();
    let expiration = boson_api::as_ms!(SystemTime::now()) + 60000;
    let ticket = InviteTicket::create(
        &inviter,
        channel_id,
        session_id,
        Some(invitee),
        expiration as u64,
        vec![7; 64],
    )
    .unwrap();

    assert!(ticket.is_named_ticket());
    assert!(ticket.is_genuine());
    assert!(!ticket.is_expired());

    let encoded = serde_json::to_vec(&ticket).unwrap();
    let decoded = serde_json::from_slice::<InviteTicket>(&encoded).unwrap();
    assert_eq!(decoded.channel_id(), &channel_id);
    assert_eq!(decoded.session_id(), &session_id);
    assert_eq!(decoded.invitee(), Some(&invitee));
    assert!(decoded.is_genuine());

    let json = ticket.to_string();
    assert!(json.contains("\"sig\":\""));
    assert!(json.contains("\"sk\":\""));
}

#[test]
fn test_revise() {
    let inviter = CryptoIdentity::new();
    let ticket = InviteTicket::create(
        &inviter,
        Id::random(),
        Id::random(),
        None,
        u64::MAX,
        vec![1; 64],
    )
    .unwrap();

    let revised = ticket.revise(vec![2; 64]).unwrap();
    assert!(revised.is_bearer_ticket());
    assert!(revised.is_genuine());
    assert_eq!(revised.session_key(), &[2; 64]);
}
