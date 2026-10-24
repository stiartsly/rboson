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

use crate::msg::{FindValueRequest, LookupRequest};
use boson_api::Id;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let value_id = Id::random();
        let expected_seq = 7;
        let msg = FindValueRequest::new(value_id.clone(), true, false, expected_seq);

        assert_eq!(msg.target(), &value_id);
        assert_eq!(msg.expected_seq(), expected_seq);

        assert!(msg.want4());
        assert!(!msg.want6());
        assert!(!msg.want_token());

        let encoded = serde_cbor::to_vec(&msg).expect("Serialization failed");
        let decoded: FindValueRequest =
            serde_cbor::from_slice(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.target(), &value_id);
        assert_eq!(decoded.expected_seq(), expected_seq);

        assert!(decoded.want4());
        assert!(!decoded.want6());
        assert!(!decoded.want_token());
    }

    #[test]
    fn test_serde_without_expected_seq() {
        let value_id = Id::random();

        let msg = FindValueRequest::new(value_id.clone(), false, true, -1);

        assert_eq!(msg.target(), &value_id);
        assert_eq!(msg.expected_seq(), -1);

        assert!(!msg.want4());
        assert!(msg.want6());
        assert!(!msg.want_token());

        let encoded = serde_cbor::to_vec(&msg).expect("Serialization failed");
        let decoded: FindValueRequest =
            serde_cbor::from_slice(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.target(), &value_id);
        assert_eq!(decoded.expected_seq(), -1);

        assert!(!decoded.want4());
        assert!(decoded.want6());
        assert!(!decoded.want_token());
    }
}
