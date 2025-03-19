use std::str::FromStr;
use boson::{
    Id,
    id::ID_BYTES,
    id::distance,
    signature,
    cryptobox,
};

use crate::{
    randomize_bytes,
    create_random_bytes,
};

/*  APIs for testcase
 - Id::random()             [X]
 - Id::default()
 - Id::from_bytes(..)       [V]
 - Id::try_from_hexstr(..)
 - Id::try_from_base58(..)
 - Id::min()
 - Id::max()
 - to_hexstr()
 - to_base58()
 - to_signature_key()
 - to_encryption_key()
 - distance(..)
 - size()
 - as_bytes()
 - Id::try_from(&[u8])
 - Id::try_from(&str)
 - Id::from(signature::PublicKey)
 - Id::from_str(&str)
 - Eq
 - PartialEq
 */

 #[test]
 fn test_default() {
    let mut bytes = [0u8; ID_BYTES];
    bytes.fill(0);

    let id1 = Id::from_bytes(bytes);
    assert_eq!(id1.size(), ID_BYTES);
    assert_eq!(id1.as_bytes(), bytes.as_slice());

    let id2 = Id::default();
    assert_eq!(id2.size(), ID_BYTES);
    assert_eq!(id2.as_bytes(), bytes.as_slice());
    assert_eq!(id1, id2);
 }

#[test]
fn test_from_bytes() {
    let mut bytes = [0u8; ID_BYTES];
    randomize_bytes::<ID_BYTES>(&mut bytes);

    let id = Id::from_bytes(bytes);
    assert_eq!(id.size(), ID_BYTES);
    assert_eq!(id.as_bytes(), bytes.as_slice());
}

#[test]
fn test_try_from_hex() {
    let hex_str = "71e1b2ecdf528b623192f899d984c53f2b13508e21ccd53de5d7158672820636";
    let id = Id::try_from_hexstr(hex_str);
    assert_eq!(id.is_ok(), true);
    assert_eq!(id.as_ref().unwrap().size(), ID_BYTES);
    assert_eq!(id.as_ref().unwrap().to_hexstr(), hex_str);
}

#[test]
fn test_try_from_base58() {
    let base58 = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let id1 = Id::try_from_base58(base58);
    assert_eq!(id1.is_ok(), true);
    assert_eq!(id1.as_ref().unwrap().size(), ID_BYTES);
    assert_eq!(id1.as_ref().unwrap().to_base58(), base58);

    let id2 = Id::try_from(base58);
    assert_eq!(id2.is_ok(), true);
    assert_eq!(id2.as_ref().unwrap().size(), ID_BYTES);
    assert_eq!(id2.as_ref().unwrap().to_base58(), base58);
    assert_eq!(id1.as_ref().unwrap(), id2.as_ref().unwrap());
}

#[test]
fn test_min() {
    let mut bytes = [0u8; ID_BYTES];
    bytes.fill(0);
    let id  = Id::from_bytes(bytes);
    let min = Id::min();
    assert_eq!(min.size(), ID_BYTES);
    assert_eq!(min.as_bytes(), bytes.as_slice());
    assert_eq!(min.to_hexstr(), "0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(min,id);
}

#[test]
fn test_max() {
    let mut bytes = [0u8; ID_BYTES];
    bytes.fill(0xFF);
    let id = Id::from_bytes(bytes);
    let max = Id::max();
    assert_eq!(max.size(), ID_BYTES);
    assert_eq!(max.as_bytes(), bytes.as_slice());
    assert_eq!(max.to_hexstr(), "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    assert_eq!(max, id);
}

#[test]
fn test_try_from_bytes_error() { // invalid length.
    let bytes = create_random_bytes(45);
    let id = Id::try_from(bytes.as_slice());
    assert_eq!(id.is_ok(), false);

    let bytes = create_random_bytes(25);
    let id = Id::try_from(bytes.as_slice());
    assert_eq!(id.is_ok(), false);
}

#[test]
fn test_trait_try_from_bytes() {
    let bytes = create_random_bytes(ID_BYTES);
    let id = Id::try_from(bytes.as_slice());
    assert_eq!(id.is_ok(), true);
    assert_eq!(id.as_ref().unwrap().size(), ID_BYTES);
    assert_eq!(id.as_ref().unwrap().as_bytes(), bytes);
}

#[test]
fn test_trait_try_from_base58_error() { // Wrong base58 encoded string.
    let base58 = "OZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let id_from = Id::try_from(base58);
    let id_into: Result<Id,_> = base58.try_into();
    assert_eq!(id_from.is_ok(), false);
    assert_eq!(id_into.is_ok(), false);
}

#[test]
fn test_trait_try_from_base58() {
    let base58 = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let id_from = Id::try_from(base58);
    let id_into: Result<Id,_> = base58.try_into();
    assert_eq!(id_from.is_ok(), true);
    assert_eq!(id_from.as_ref().unwrap().to_base58(), base58);
    assert_eq!(id_into.is_ok(), true);
    assert_eq!(id_into.as_ref().unwrap().to_base58(), base58);
    assert_eq!(id_from.unwrap(), id_into.unwrap());
}

#[test]
fn test_trait_from_str() {
    let base58 = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let id_from = Id::from_str(base58);
    let id_parsed1: Result<Id, _> = base58.parse();
    let id_parsed2 = base58.parse::<Id>();
    assert_eq!(id_from.is_ok(), true);
    assert_eq!(id_from.as_ref().unwrap().to_base58(), base58);
    assert_eq!(id_parsed1.is_ok(), true);
    assert_eq!(id_parsed2.is_ok(), true);
    assert_eq!(id_parsed1.as_ref().unwrap().to_base58(), base58);
    assert_eq!(id_parsed2.as_ref().unwrap().to_base58(), base58);
    assert_eq!(id_from.as_ref().unwrap().clone(), id_parsed1.unwrap());
    assert_eq!(id_from.unwrap(), id_parsed2.unwrap());
}

#[test]
fn test_trait_try_from_signature_publickey() {
    let kp = signature::KeyPair::random();
    let pk = kp.to_public_key();
    let encryption_pk: cryptobox::PublicKey = kp.public_key().try_into().unwrap();
    let id_from = Id::from(pk.clone());
    let id_into: Id = pk.clone().into();
    assert_eq!(id_from.to_signature_key(), pk.clone());
    assert_eq!(id_into.to_signature_key(), pk.clone());
    assert_eq!(id_from.to_encryption_key(), encryption_pk);
    assert_eq!(id_from, id_into);
}

#[test]
fn test_id_eq() {
    let hex_str = "71e1b2ecdf528b623192f899d984c53f2b13508e21ccd53de5d7158672820636";
    let id1 = Id::try_from_hexstr(hex_str).expect("Invalid hex Id");
    let id2 = Id::try_from_hexstr(hex_str).expect("Invalid hex Id");
    assert_eq!(id1, id2);
}

#[test]
fn test_distance() {
    let id1_str = "00000000f528d6132c15787ed16f09b08a4e7de7e2c5d3838974711032cb7076";
    let id2_str = "00000000f0a8d6132c15787ed16f09b08a4e7de7e2c5d3838974711032cb7076";
    let dist_str = "0000000005800000000000000000000000000000000000000000000000000000";
    let id1 = Id::try_from_hexstr(id1_str);
    let id2 = Id::try_from_hexstr(id2_str);
    assert_eq!(id1.is_ok(), true);
    assert_eq!(id2.is_ok(), true);

    let id1_unwrap = id1.unwrap();
    let id2_unwrap = id2.unwrap();
    assert_eq!(distance(&id1_unwrap, &id2_unwrap).to_hexstr(), dist_str);
    assert_ne!(id1_unwrap, id2_unwrap);
}
