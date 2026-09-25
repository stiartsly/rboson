use crate::core::Id;
use std::cmp::Ordering;

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
}
