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

use crate::msg::{FindNodeRequest, LookupRequest};
use boson_api::Id;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let nodeid = Id::random();
        let req = FindNodeRequest::new(nodeid.clone(), true, false, true);

        let encoded = serde_cbor::to_vec(&req).expect("Serialization failed");

        println!("11encoded (hex): 0x{}", hex::encode(&encoded));
        let decoded =
            serde_cbor::from_slice::<FindNodeRequest>(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.target(), &nodeid);
        assert_eq!(decoded.want(), 0x05);

        assert!(decoded.want4());
        assert!(!decoded.want6());
        assert!(decoded.want_token());
    }

    #[test]
    fn test_serde_without_token() {
        let nodeid = Id::try_from("GfVNV6f5U2fnp6yZS7FjNLGkheyfDSpufK3uQaofJyab").unwrap();
        println!(">> nodeid: {}", nodeid);
        let req = FindNodeRequest::new(nodeid.clone(), true, true, false);

        let encoded = serde_cbor::to_vec(&req).expect("Serialization failed");
        println!("22encoded (hex): 0x{}", hex::encode(&encoded));
        let decoded =
            serde_cbor::from_slice::<FindNodeRequest>(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.target(), &nodeid);
        assert_eq!(decoded.want(), 0x03);

        assert!(decoded.want4());
        assert!(decoded.want6());
        assert!(!decoded.want_token());
    }
}
