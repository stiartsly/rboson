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

use std::cmp::Ordering;
use crate::Id;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_three_way_compare() {
        let id0 =
            Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8ca214a3d09b6676cb8")
                .expect("invalid hex Id");
        let id1 =
            Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8")
                .expect("invalid hex Id");
        let id2 =
            Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a885a8ca214a3d09b6676cb8")
                .expect("invalid hex Id");

        assert_eq!(id0.three_way_compare(&id1, &id2), Ordering::Less);
        assert_eq!(id0.three_way_compare(&id1, &id1), Ordering::Equal);
    }

    #[test]
    fn test_bites_equal() {
        let id1 =
            Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8")
                .expect("invalid hex Id");
        let id2 =
            Id::try_from("0x4833af415166cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8")
                .expect("invalid hex Id");

        for i in 0..45 {
            assert_eq!(Id::bits_equal(&id1, &id2, i), true);
        }
        for i in 45..Id::BITS as i32 {
            assert_eq!(Id::bits_equal(&id1, &id2, i), false);
        }

        let id2 = id1.clone();
        for i in 0..Id::BITS as i32 {
            assert_eq!(Id::bits_equal(&id1, &id2, i), true);
        }

        let id2 =
            Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb9")
                .expect("invalid hex Id");
        for i in 0..Id::BITS as i32 - 1 {
            assert_eq!(Id::bits_equal(&id1, &id2, i), true);
        }
        let result = Id::bits_equal(&id1, &id2, Id::BITS as i32 - 1);
        assert_eq!(result, false);
    }

    #[test]
    fn test_bites_copy() {
        let id1 =
            Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8")
                .expect("invalid hex Id");
        for i in 0..Id::BITS as i32 {
            let mut id2 = Id::random();
            Id::bits_copy(&id1, &mut id2, i);
            assert_eq!(Id::bits_equal(&id1, &id2, i), true)
        }
    }

    #[test]
    fn test_shift() {
        let v = (-0x80 as i8) >> 2;
        let u = (0x80 as u8) >> 2;
        assert_eq!(v as u8, 224);
        assert_eq!(u as u8, 32);
    }

    #[test]
    fn test_serde_cbor() {
        let id_str = "0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let id = Id::try_from(id_str).expect("invalid hex Id");
        let ser = serde_cbor::to_vec(&id).expect("failed to serialize Id to CBOR");
        let des: Id = serde_cbor::from_slice(&ser).expect("failed to deserialize Id from CBOR");
        let id2_str = des.to_hexstr();
        assert_eq!(id_str, id2_str);
        assert_eq!(id, des);
    }

    #[test]
    fn test_serde_json() {
        let id_str = "0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let id = Id::try_from(id_str).expect("invalid hex Id");
        let ser = serde_json::to_string(&id).expect("failed to serialize Id to JSON");
        let des: Id = serde_json::from_str(&ser).expect("failed to deserialize Id from JSON");
        let id2_str = des.to_hexstr();
        assert_eq!(id_str, id2_str);
        assert_eq!(id, des);
    }

    #[test]
    fn test_base58_invalid_lengths() {
        // Short base58 should fail
        assert!(Id::try_from_base58("1").is_err());
        assert!(Id::try_from_base58("abc").is_err());
        assert!(Id::try_from_base58("").is_err());

        // Valid random Id should round-trip
        let id = Id::random();
        let b58 = id.to_base58();
        let parsed = Id::try_from_base58(&b58).expect("valid base58 should parse");
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_bit_metrics() {
        let a = Id::try_from("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let b = Id::try_from("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        assert_eq!(a.add(&b).to_hexstr(), "0x0000000000000000000000000000000000000000000000000000000000000003");
        assert_eq!(a.distance(&b).get_trailing_zeros(), 0);
        assert_eq!(a.get_leading_zeros(), 255);
        assert_eq!(a.approx_distance(&b), 2);
        assert_eq!(a.get_int(28).unwrap(), 1);
        assert_eq!(a.to_binary_string().len(), Id::BITS);
        assert_eq!(a.to_binary_string_with_spaces().split(' ').count(), Id::BYTES);
    }

    #[test]
    fn test_parses_hexstr() {
        let id = Id::random();
        assert_eq!(Id::try_from(id.to_did_string().as_str()).unwrap(), id);
        assert_eq!(Id::try_from_hexstr(&id.to_hexstr()).unwrap(), id);
        assert_eq!(Id::try_from_base58(&id.to_base58()).unwrap(), id);
        assert_eq!(Id::try_from_bytes_at(&[0u8; 4].iter().chain(id.as_bytes()).copied().collect::<Vec<_>>(), 4).unwrap(), id);
    }

    #[test]
    fn encryption_key_of_an_invalid_point_is_an_error() {
        // A small-order point, which libsodium refuses to convert to X25519: an error, not a panic.
        let id = Id::try_from([0u8; Id::BYTES].as_slice()).unwrap();
        assert!(id.to_encryption_key().is_err());
        let _ = id.to_signature_key();

        // An id made from a real key pair always converts; Id::random() is random bytes, not a key,
        // and about half of all byte strings are not valid points.
        let id = Id::from(crate::signature::KeyPair::random().public_key());
        assert!(id.to_encryption_key().is_ok());
    }
}
