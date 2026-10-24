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

use boson_api::{cryptobox::Nonce, Id, Value};
use crate::{msg::store_value_req::StoreValueRequest};

fn make_value() -> Value {
    Value::packed(
        Some(Id::random()),
        None,
        Some(Nonce::random()),
        Some(vec![9; 64]),
        vec![1, 2, 3, 4],
        7,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let value = make_value();
        let req = StoreValueRequest::new(value.clone(), 42, 11);
        assert_eq!(req.token(), 42);
        assert_eq!(req.expected_seq(), 11);
        assert_eq!(req.value(), &value);

        let encoded = serde_cbor::to_vec(&req).expect("Serialization failed");
        println!("encoded: {}", hex::encode(&encoded));
        let decoded: StoreValueRequest =
            serde_cbor::from_slice(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), 11);
        assert_eq!(decoded.value(), &value);
    }

    #[test]
    fn test_serde_no_expected_seq() {
        let value = make_value();
        let req = StoreValueRequest::new(value.clone(), 42, -1);

        assert_eq!(req.token(), 42);
        assert_eq!(req.expected_seq(), -1);
        assert_eq!(req.value(), &value);

        let encoded = serde_cbor::to_vec(&req).expect("Serialization failed");
        println!("encoded: {}", hex::encode(&encoded));
        let decoded: StoreValueRequest =
            serde_cbor::from_slice(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.token(), 42);
        assert_eq!(decoded.expected_seq(), -1);
        assert_eq!(decoded.value(), &value);
    }
}
