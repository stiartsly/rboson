use std::cmp::Ordering;
use crate::core::{
    id,
    Id,
    ID_BITS,
};

#[test]
fn test_three_way_compare() {
    let id0 = Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8ca214a3d09b6676cb8").expect("invalid hex Id");
    let id1 = Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8").expect("invalid hex Id");
    let id2 = Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a885a8ca214a3d09b6676cb8").expect("invalid hex Id");

    assert_eq!(id0.three_way_compare(&id1, &id2), Ordering::Less);
    assert_eq!(id0.three_way_compare(&id1, &id1), Ordering::Equal);
}

#[test]
fn test_bites_equal() {
    let id1 = Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8").expect("invalid hex Id");
    let id2 = Id::try_from("0x4833af415166cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8").expect("invalid hex Id");

    for i in 0..45 {
        assert_eq!(id::bits_equal(&id1, &id2, i), true);
    }
    for i in 45.. ID_BITS as i32 {
        assert_eq!(id::bits_equal(&id1, &id2, i), false);
    }

    let id2 = id1.clone();
    for i in 0.. ID_BITS as i32 {
        assert_eq!(id::bits_equal(&id1, &id2, i), true);
    }

    let id2 = Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb9").expect("invalid hex Id");
    for i in 0..ID_BITS as i32 -1 {
        assert_eq!(id::bits_equal(&id1, &id2, i), true);
    }
    let result = id::bits_equal(&id1, &id2, ID_BITS as i32 -1);
    assert_eq!(result, false);
}

#[test]
fn test_bites_copy() {
    let id1 = Id::try_from("0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8").expect("invalid hex Id");
    for i in 0.. ID_BITS as i32 {
        let mut id2 = Id::random();
        id::bits_copy(&id1, &mut id2, i);
        assert_eq!(id::bits_equal(&id1, &id2, i), true)
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
fn test_serde() {
    let id1_str = "0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
    let id1 = Id::try_from(id1_str).expect("invalid hex Id");
    let cbor = serde_cbor::to_vec(&id1).expect("failed to serialize Id to CBOR");
    let id2: Id = serde_cbor::from_slice(&cbor).expect("failed to deserialize Id from CBOR");
    let id2_str = id2.to_hexstr();
    assert_eq!(id1_str, id2_str);
    assert_eq!(id1, id2);

    let id1_b58 = id1.to_base58();
    let json = serde_json::to_string(&id1_b58).expect("failed to serialize Id to JSON");
    let id3: Id = serde_json::from_str(&json).expect("failed to deserialize Id from JSON");
    let id3_b58 = id3.to_base58();
    assert_eq!(id1, id3);
    assert_eq!(id1_b58, id3_b58);
}
