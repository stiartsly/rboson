use std::cmp::Ordering;
use crate::core::id;
use crate::{
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
