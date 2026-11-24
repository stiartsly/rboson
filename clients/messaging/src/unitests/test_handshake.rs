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

use crate::internal::handshake::*;

#[test]
fn test_friend_request() {
    let timestamp = 42_i64;
    let hello = Some("hello from the other side".to_string());
    let handshake = Handshake::friend_request(hello.clone(), timestamp);

    assert_eq!(handshake.timestamp, timestamp);
    assert_eq!(handshake.body, HandshakeType::FriendRequest(hello.clone()));
    assert!(handshake.from.is_none());

    let encoded = handshake.serialize();
    let decoded = Handshake::deserialize(&encoded)
            .expect("friend request should deserialize");

    assert_eq!(decoded.timestamp, timestamp);
    assert_eq!(decoded.body, HandshakeType::FriendRequest(hello));
    assert!(decoded.from.is_none());
}

#[test]
fn test_friend_request_accept() {
    let timestamp = 43_i64;
    let session_key = vec![7_u8; 64];
    let handshake = Handshake::friend_request_accept(session_key.clone(), timestamp);

    assert_eq!(handshake.timestamp, timestamp);
    assert_eq!(
        handshake.body,
        HandshakeType::FriendRequestAccept(session_key.clone())
    );
    assert!(handshake.from.is_none());

    let encoded = handshake.serialize();
    let decoded = Handshake::deserialize(&encoded)
            .expect("friend request accept should deserialize");

    assert_eq!(decoded.timestamp, timestamp);
    assert_eq!(decoded.body, HandshakeType::FriendRequestAccept(session_key));
    assert!(decoded.from.is_none());
}
