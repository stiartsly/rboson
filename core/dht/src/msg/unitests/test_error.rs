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

use crate::msg::error::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let err = Error::new(500, "example error");
        assert_eq!(err.code(), 500);
        assert_eq!(err.description(), "example error");

        let encoded = serde_cbor::to_vec(&err).expect("Serialization failed");
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded = serde_cbor::from_slice::<Error>(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.code(), 500);
        assert_eq!(decoded.description(), "example error");
    }

    #[test]
    fn test_serde_with_empty_description() {
        let err = Error::new(404, "");
        assert_eq!(err.code(), 404);
        assert_eq!(err.description(), "");

        let encoded = serde_cbor::to_vec(&err).expect("Serialization failed");
        // println!("encoded: {}", hex::encode(&encoded));
        let decoded = serde_cbor::from_slice::<Error>(&encoded).expect("Deserialization failed");

        assert_eq!(decoded.code(), 404);
        assert_eq!(decoded.description(), "");
    }
}
