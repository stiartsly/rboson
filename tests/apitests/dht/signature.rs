use boson::{
    Id,
    signature,
    signature::{
        PrivateKey,
        PublicKey,
        KeyPair,
        Signature
    },
    Error
};

use crate::create_random_bytes;

/*
 # PrivateKey APIs.
    - try_from(&str)
    - try_from(&[u8])
    - size()
    - as_bytes()
    - clear()
    - sign(data, signature) -> Result(usize)
    - sign_into(data) -> Result<Vec<u8>>
 */

#[test]
fn test_sk_from_str() {
    let sk: Result<PrivateKey, Error> = "a3218958b88d86dead1a58b439a22c161e0573022738b570210b123dc0b046faec6f3cd4ed1e6801ebf33fd60c07cf9924ef01d829f3f5af7377f054bff31501".try_into();
    assert_eq!(sk.is_ok(), true);
    let sk = sk.unwrap();
    assert_eq!(sk.size(), PrivateKey::BYTES);
    assert_eq!(sk.as_bytes().len(), PrivateKey::BYTES);
}

#[test]
fn test_sk_tryfrom_bytes() {
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    assert_eq!(sk.is_ok(), true);
    assert_eq!(sk.as_ref().unwrap().size(), PrivateKey::BYTES);
    assert_eq!(sk.as_ref().unwrap().as_bytes(), bytes);
}

#[test]
fn test_sk_clear() {
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    assert_eq!(sk.is_ok(), true);
    let zero_bytes = vec![0u8; PrivateKey::BYTES];

    let mut sk = sk.unwrap();
    sk.clear();
    assert_eq!(sk.size(), PrivateKey::BYTES);
    assert_eq!(sk.as_bytes(), zero_bytes);
}

#[test]
fn test_sk_sign() {
    let plain = create_random_bytes(256);
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    assert_eq!(sk.is_ok(), true);

    let mut sig = vec![0u8; Signature::BYTES];
    let sk = sk.unwrap();
    let rc = sk.sign(&plain, &mut sig.as_mut());
    assert_eq!(rc.is_ok(), true);
    assert_eq!(rc.unwrap(), Signature::BYTES);
}

#[test]
fn test_sk_sign_into() {
    let plain = create_random_bytes(256);
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    assert_eq!(sk.is_ok(), true);

    let sk = sk.unwrap();
    let rc = sk.sign_into(&plain);
    assert_eq!(rc.is_ok(), true);

    let sig = rc.unwrap();
    assert_eq!(sig.len(), Signature::BYTES);
}

/*
 # PublicKey APIs.
    - try_from(&[u8])
    - size()
    - as_bytes()
    - clear()
    - verify(data, signature) -> Result<()>
 */
#[test]
fn test_pk_tryfrom_bytes() {
    let bytes = create_random_bytes(PublicKey::BYTES);
    let pk = PublicKey::try_from(bytes.as_ref());
    assert_eq!(pk.is_ok(), true);
    assert_eq!(pk.as_ref().unwrap().size(), PublicKey::BYTES);
    assert_eq!(pk.as_ref().unwrap().as_bytes(), bytes);
}

#[test]
fn test_pk_clear() {
    let bytes = create_random_bytes(PublicKey::BYTES);
    let pk = PublicKey::try_from(bytes.as_slice());
    assert_eq!(pk.is_ok(), true);
    let zero_bytes = vec![0u8; PublicKey::BYTES];

    let mut pk = pk.unwrap();
    pk.clear();
    assert_eq!(pk.size(), PublicKey::BYTES);
    assert_eq!(pk.as_bytes(), zero_bytes);
}

#[test]
fn test_pk_verify1() {
    let kp = KeyPair::random();

    let plain = create_random_bytes(256);
    let rc = kp.private_key().sign_into(&plain);
    assert_eq!(rc.is_ok(), true);

    let sig = rc.unwrap();
    assert_eq!(sig.len(), Signature::BYTES);

    let rc = kp.public_key().verify(&plain, &sig);
    assert_eq!(rc.is_ok(), true);
}

#[test]
fn test_pk_verify2() {
    let kp = KeyPair::new();

    let plain = create_random_bytes(256);
    let rc = kp.private_key().sign_into(&plain);
    assert_eq!(rc.is_ok(), true);

    let sig = rc.unwrap();
    assert_eq!(sig.len(), Signature::BYTES);

    let rc = kp.public_key().verify(&plain, &sig);
    assert_eq!(rc.is_ok(), true);
}

#[test]
fn test_pk_into_id() {
    let kp = KeyPair::new();
    let id: Id= kp.to_public_key().into();
    let pk = id.to_signature_key();
    assert_eq!(kp.to_public_key(), pk);
}

/*
# KeyPair APIs.
- try_from(&[u8])
- from(&PrivateKey)
- from(&siganture::KeyPair)
- new()
- random()
- try_from_seed(&[u8])
- private_key()
- to_private_key()
- public_key()
- to_pubic_key()
- clear()
*/

#[test]
fn test_keypair_trait_tryfrom_bytes() {
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    assert_eq!(sk.is_ok(), true);

    let kp = KeyPair::try_from(sk.unwrap().as_bytes());
    if let Err(e) = kp.as_ref() {
    println!("e:{}",e);
    }
    assert_eq!(kp.is_ok(), true);
    let kp = kp.unwrap();
    assert_eq!(kp.private_key().clone(), kp.to_private_key());
    assert_eq!(kp.public_key().clone(), kp.to_public_key());
}

#[test]
fn test_keypair_trait_tryfrom_sk() {
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    let kp = KeyPair::from(&sk.unwrap());
    assert_eq!(kp.private_key().clone(), kp.to_private_key());
    assert_eq!(kp.public_key().clone(), kp.to_public_key());
}

#[test]
fn test_keypair_new() {
    let kp = KeyPair::new();
    assert_eq!(kp.private_key().clone(), kp.to_private_key());
    assert_eq!(kp.public_key().clone(), kp.to_public_key());
}

#[test]
fn test_keypair_random() {
    let kp = KeyPair::random();
    assert_eq!(kp.private_key().clone(), kp.to_private_key());
    assert_eq!(kp.public_key().clone(), kp.to_public_key());
}

#[test]
fn test_keypair_tryfrom_seeds() {
     let bytes = create_random_bytes(KeyPair::SEED_BYTES);
     let kp = KeyPair::try_from_seed(bytes.as_slice());
     assert_eq!(kp.is_ok(), true);
     let kp = kp.unwrap();
     assert_eq!(kp.private_key().clone(), kp.to_private_key());
     assert_eq!(kp.public_key().clone(), kp.to_public_key());
}

#[test]
fn test_keypair_clear() {
    let mut kp = KeyPair::random();
    let zero_bytes = vec![0u8; 256];

    kp.clear();
    assert_eq!(*kp.public_key().as_bytes(), zero_bytes[..PublicKey::BYTES]);
    assert_eq!(*kp.private_key().as_bytes(), zero_bytes[..PrivateKey::BYTES]);
}

/*
 * Signature APIs
 - new()
 - reset(),
 - update(),
 - sign(sig, sk) -> Result<usize>
 - sign_into(sk) -> Result<Vec<u8>>
 - verify(sig, pk) -> Result<()>
 */
#[test]
fn test_signature() {
    let kp = KeyPair::random();
    let mut sig = Signature::new();
    let data1 = create_random_bytes(32);
    let data2 = create_random_bytes(64);
    let data3 = create_random_bytes(128);
    sig.update(data1.as_slice())
        .update(data2.as_slice())
        .update(data3.as_slice());

    let mut sig_data = vec![0u8; Signature::BYTES];
    let rc = sig.sign(sig_data.as_mut_slice(), kp.private_key());
    assert_eq!(rc.is_ok(), true);
    assert_eq!(rc.unwrap(), Signature::BYTES);

   // let rc = sig.verify(&sig_data, kp.public_key());
   // TODO: assert_eq!(rc.is_ok(), true);
}

/**
 Glogal APIs.
 - sign(data, sig, sk) -> Result<usize>
 - sign_into(data, sk) -> Result<Vec<u8>>
 - verify(data, sig, pk) -> Result<()>
 */
#[test]
fn test_signature_sign() {
    let kp = KeyPair::random();
    let data = create_random_bytes(256);
    let mut sig = vec![0u8; Signature::BYTES];
    let rc = signature::sign(&data, sig.as_mut(), kp.private_key());
    assert_eq!(rc.is_ok(), true);
    assert_eq!(rc.unwrap(), sig.len());

    let rc = signature::verify(&data, &sig, kp.public_key());
    assert_eq!(rc.is_ok(), true);
}

#[test]
fn test_signature_sign_into() {
    let kp = KeyPair::random();
    let data = create_random_bytes(256);
    let rc = signature::sign_into(&data, kp.private_key());
    assert_eq!(rc.is_ok(), true);

    let sig = rc.unwrap();
    assert_eq!(sig.len(), Signature::BYTES);

    let rc = signature::verify(&data, &sig, kp.public_key());
    assert_eq!(rc.is_ok(), true);
}
