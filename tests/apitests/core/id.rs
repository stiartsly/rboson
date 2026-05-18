use boson::{
    id,
    Id,
    signature,
    cryptobox
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
 - Eq
 - PartialEq
 */

 #[test]
 fn test_default() {
    let zeros = [0u8; Id::BYTES];

    let id1 = Id::from_bytes(zeros);
    let id2 = Id::default();
    assert_eq!(id2.size(), Id::BYTES);
    assert_eq!(id2.as_bytes(), zeros.as_slice());
    assert_eq!(id1, id2);
 }

#[test]
fn test_from_bytes() {
    let mut bytes = [0u8; Id::BYTES];
    randomize_bytes(&mut bytes);

    let id = Id::from_bytes(bytes);
    assert_eq!(id.size(), Id::BYTES);
    assert_eq!(id.as_bytes(), bytes.as_slice());

    let result = Id::try_from(bytes.as_slice());
    assert_eq!(result.is_ok(), true);
    let id2 = result.unwrap();
    assert_eq!(id2.size(), Id::BYTES);
    assert_eq!(id2.as_bytes(), bytes.as_slice());
    assert_eq!(id, id2);
}

#[test]
fn test_try_from_hex_str() {
    let hexstr = "71e1b2ecdf528b623192f899d984c53f2b13508e21ccd53de5d7158672820636";
    let result = Id::try_from(hexstr);
    assert_eq!(result.is_ok(), false);
    println!("Error: {}", result.err().unwrap());

    let hexstr = "0x71e1b2ecdf528b623192f899d984c53f2b13508e21ccd53de5d7158672820636";
    let result = Id::try_from(hexstr);
    assert_eq!(result.is_ok(), true);
    let id1 = result.unwrap();
    assert_eq!(id1.size(), Id::BYTES);
    assert_eq!(id1.to_hexstr(), hexstr);

    let result = Id::try_from(hexstr);
    assert_eq!(result.is_ok(), true);
    let id2 = result.unwrap();
    assert_eq!(id2.size(), Id::BYTES);
    assert_eq!(id2.to_hexstr(), hexstr);
    assert_eq!(id1, id2);
}

#[test]
fn test_try_from_base58_str() {
    let base58 = "0xHZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let result = Id::try_from(base58);
    assert_eq!(result.is_ok(), false);

    let base58 = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let result = Id::try_from(base58);
    assert_eq!(result.is_ok(), true);

    let id1 = result.unwrap();
    assert_eq!(id1.size(), Id::BYTES);
    assert_eq!(id1.to_base58(), base58);

    let result = Id::try_from(base58);
    assert_eq!(result.is_ok(), true);
    let id2 = result.unwrap();
    assert_eq!(id2.size(), Id::BYTES);
    assert_eq!(id2.to_base58(), base58);
    assert_eq!(id1, id2);
}

#[test]
fn test_min() {
    let mut bytes = [0u8; Id::BYTES];
    bytes.fill(0);
    let id1  = Id::from_bytes(bytes);
    let id2 = Id::min();
    let id3 = Id::default();
    let id4 = Id::zero();
    assert_eq!(id1.size(), Id::BYTES);
    assert_eq!(id1.as_bytes(), bytes.as_slice());
    assert_eq!(id1.to_hexstr(), "0x0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(id1, id2);
    assert_eq!(id1, id3);
    assert_eq!(id1, id4);
}

#[test]
fn test_max() {
    let mut bytes = [0u8; Id::BYTES];
    bytes.fill(0xFF);
    let id1 = Id::from_bytes(bytes);
    let id2 = Id::max();
    assert_eq!(id1.size(), Id::BYTES);
    assert_eq!(id1.as_bytes(), bytes.as_slice());
    assert_eq!(id1.to_hexstr(), "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    assert_eq!(id1, id2);
}

#[test]
fn test_try_from_bytes() {
    // wrong size of bytes
    let bytes = create_random_bytes(45);
    let result = Id::try_from(bytes.as_slice());
    assert_eq!(result.is_ok(), false);

    let bytes = create_random_bytes(25);
    let result = Id::try_from(bytes.as_slice());
    assert_eq!(result.is_ok(), false);

    // correct size of bytes
    let bytes = create_random_bytes(Id::BYTES);
    let result = Id::try_from(bytes.as_slice());
    assert_eq!(result.is_ok(), true);

    let id = result.unwrap();
    assert_eq!(id.size(), Id::BYTES);
    assert_eq!(id.as_bytes(), bytes.as_slice());
}

#[test]
fn test_try_from_base58() {
    let base58 = "OZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let result = Id::try_from(base58);
    assert_eq!(result.is_ok(), false);

    let result: Result<Id,_> = base58.try_into();
    assert_eq!(result.is_ok(), false);

    let base58 = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let result = Id::try_from(base58);
    assert_eq!(result.is_ok(), true);

    let id_from = result.unwrap();
    assert_eq!(id_from.size(), Id::BYTES);
    assert_eq!(id_from.to_base58(), base58);

    let result: Result<Id,_> = base58.try_into();
    assert_eq!(result.is_ok(), true);

    let id_into = result.unwrap();
    assert_eq!(id_into.size(), Id::BYTES);
    assert_eq!(id_into.to_base58(), base58);
    assert_eq!(id_from, id_into);
}

#[test]
fn test_try_from_str() {
    let base58 = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
    let result = Id::try_from(base58);
    assert_eq!(result.is_ok(), true);

    let id1 = result.unwrap();
    assert_eq!(id1.size(), Id::BYTES);
    assert_eq!(id1.to_base58(), base58);

    let hexstr = "0xf610197a374dfc3801cb00ad1acda6f7a52bb8e0bff10a3e0db2aacdcea41ed8";
    let result = Id::try_from(hexstr);
    assert_eq!(result.is_ok(), true);
    let id2 = result.unwrap();
    assert_eq!(id2.size(), Id::BYTES);
    assert_eq!(id2.to_hexstr(), hexstr);
    assert_eq!(id1.to_hexstr(), hexstr);
    assert_eq!(id2.to_base58(), base58);
    assert_eq!(id1, id2);

    let id3: Id = base58.parse().unwrap();
    let id4: Id = hexstr.parse().unwrap();
    assert_eq!(id1, id3);
    assert_eq!(id1, id4);
}

#[test]
fn test_try_from_signature_publickey() {
    let kp = signature::KeyPair::random();
    let sig_pk = kp.to_public_key();
    let enc_pk: cryptobox::PublicKey = kp.public_key().try_into().unwrap();

    let id = Id::from(&sig_pk);
    assert_eq!(id.to_signature_key(), sig_pk);
    assert_eq!(id.to_encryption_key(), enc_pk);
}

#[test]
fn test_id_eq() {
    let hexstr = "0x71e1b2ecdf528b623192f899d984c53f2b13508e21ccd53de5d7158672820636";
    let id1 = Id::try_from(hexstr).expect("Invalid hex Id");
    let id2 = Id::try_from(hexstr).expect("Invalid hex Id");
    assert_eq!(id1, id2);
}

#[test]
fn test_distance() {
    let id1str = "0x00000000f528d6132c15787ed16f09b08a4e7de7e2c5d3838974711032cb7076";
    let id2str = "0x00000000f0a8d6132c15787ed16f09b08a4e7de7e2c5d3838974711032cb7076";
    let dist_str = "0x0000000005800000000000000000000000000000000000000000000000000000";
    let id1 = Id::try_from(id1str).expect("Invalid hex Id");
    let id2 = Id::try_from(id2str).expect("Invalid hex Id");

    assert_eq!(Id::distance_between(&id1, &id2).to_hexstr(), dist_str);
    assert_ne!(id1, id2);
}

#[test]
fn test_ser_deser() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    #[allow(non_snake_case)]
    struct TestId {
        id: Id,
        pad: String,
    }

    let data = TestId {
        id: Id::random(),
        pad: "pad".to_string(),
    };
    let ser = serde_json::to_string(&data).unwrap();
    let des: TestId = serde_json::from_str(&ser).unwrap();
    assert_eq!(data, des);

    let ser = serde_cbor::to_vec(&data).unwrap();
    let des: TestId = serde_cbor::from_slice(&ser).unwrap();
    assert_eq!(data, des);
}

#[test]
fn test_ser_deser_option() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    #[allow(non_snake_case)]
    struct TestId {
        id: Option<Id>,
    }

    let data = TestId {
        id: Some(Id::random())
    };
    let ser = serde_json::to_string(&data).unwrap();
    let des: TestId = serde_json::from_str(&ser).unwrap();
    assert_eq!(data, des);

    let data_none = TestId {
        id: None,
    };
    let ser = serde_json::to_string(&data_none).unwrap();
    let des: TestId = serde_json::from_str(&ser).unwrap();
    assert_eq!(data_none, des);

    let ser = serde_cbor::to_vec(&data).unwrap();
    let des: TestId = serde_cbor::from_slice(&ser).unwrap();
    assert_eq!(data, des);

    let ser = serde_cbor::to_vec(&data_none).unwrap();
    let des: TestId = serde_cbor::from_slice(&ser).unwrap();
    assert_eq!(data_none, des);
}

#[test]
fn test_id_try_from_bit_at1() {
    let msb = Id::try_from_bit_at(0).expect("bit 0 should be valid");
    let lsb = Id::try_from_bit_at(Id::BITS - 1).expect("last bit should be valid");

    assert_eq!(msb.to_hexstr(), "0x8000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(lsb.to_hexstr(), "0x0000000000000000000000000000000000000000000000000000000000000001");
    assert!(Id::try_from_bit_at(Id::BITS).is_err());
}

#[test]
fn test_try_from_bit_at2() {
    let id = Id::try_from_bit_at(0).expect("bit 0 should be valid");
    let binary = id.to_binary_string();

    assert_eq!(binary.len(), Id::BITS);
    assert!(binary.starts_with('1'));
    assert!(binary[1..].chars().all(|ch| ch == '0'));
    assert_eq!(format!("{}", id), id.to_base58());
    assert_eq!(format!("{:b}", id), binary);
}

#[test]
fn test_id_try_from_base58() {
    let id = Id::try_from("0xf610197a374dfc3801cb00ad1acda6f7a52bb8e0bff10a3e0db2aacdcea41ed8")
        .expect("hex id should be valid");

    assert_eq!(id.to_abbr_hexstr(), "0xf61019...1ed8");
    assert_eq!(id.to_abbr_base58(), "HZXX...zuhZ");
    assert_eq!(id.to_abbr_str(), id.to_abbr_base58());
    assert_eq!(id.to_did_string(), format!("{}{}", id::DID_PREFIX, id.to_base58()));
}

#[test]
fn test_id_distance() {
    let min = Id::MIN_ID;
    let max = Id::MAX_ID;
    let zero = Id::zero();
    let random = Id::random();

    assert_eq!(min, Id::min());
    assert_eq!(max, Id::max());
    assert_eq!(min, zero);
    assert_eq!(min.distance(&random), random.distance(&min));
    assert_eq!(random.distance(&random), zero);
    assert_eq!(Id::distance_between(&max, &min), max);
}

#[test]
fn test_id_try_from() {
    let id = Id::random();
    let hex = id.to_hexstr();
    let base58 = id.to_base58();

    let from_hex = Id::try_from_hexstr(&hex).expect("hex string should parse");
    let from_base58 = Id::try_from_base58(&base58).expect("base58 string should parse");

    assert_eq!(from_hex, id);
    assert_eq!(from_base58, id);
    assert!(Id::try_from_hexstr("deadbeef").is_err());
    assert!(Id::try_from_base58("0xdeadbeef").is_err());
}
