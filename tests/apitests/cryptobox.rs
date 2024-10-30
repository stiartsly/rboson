use boson::core::{
    signature,
    cryptobox::{
        PrivateKey,
        PublicKey,
        Nonce,
        KeyPair,
        CryptoBox
    }
};

use crate::create_random_bytes;

/*
 # PrivateKey APIs.
    - try_from(&[u8])
    - try_from(&signature::PrivateKey)
    - size()
    - as_bytes()
    - clear()
 */
#[test]
fn test_sk_tryfrom_bytes() {
    let bytes = create_random_bytes(PrivateKey::BYTES);
    let sk = PrivateKey::try_from(bytes.as_slice());
    assert_eq!(sk.is_ok(), true);
    assert_eq!(sk.as_ref().unwrap().size(), PrivateKey::BYTES);
    assert_eq!(sk.as_ref().unwrap().as_bytes(), bytes);
}

#[test]
fn test_sk_tryfrom_signature_sk() {
    let bytes = create_random_bytes(signature::PrivateKey::BYTES);
    let signature_sk = signature::PrivateKey::try_from(bytes.as_slice()).unwrap();
    let sk = PrivateKey::try_from(&signature_sk);
    assert_eq!(sk.is_ok(), true);
    assert_eq!(sk.as_ref().unwrap().size(), PrivateKey::BYTES);
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

/*
 # PublicKey APIs.
    - try_from(&[u8])
    - try_from(&signature::PublicKey)
    - size()
    - as_bytes()
    - clear()
 */

#[test]
fn test_pk_tryfrom_bytes() {
    let bytes = create_random_bytes(PublicKey::BYTES);
    let pk = PublicKey::try_from(bytes.as_slice());
    assert_eq!(pk.is_ok(), true);
    assert_eq!(pk.as_ref().unwrap().size(), PublicKey::BYTES);
    assert_eq!(pk.as_ref().unwrap().as_bytes(), bytes);
}

#[test]
fn test_pk_tryfrom_signature_pk() {
    let kp = signature::KeyPair::random();
    let pk = PublicKey::try_from(kp.public_key());
    assert_eq!(pk.is_ok(), true);
    assert_eq!(pk.as_ref().unwrap().size(), PublicKey::BYTES);
    return;
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

 /*
 # Nonce APIs.
    - try_from(&[u8])
    - random()
    - increment()
    - size()
    - as_bytes()
    - clear()
 */

#[test]
fn test_nonce_tryfrom_bytes() {
    let bytes = create_random_bytes(Nonce::BYTES);
    let nonce = Nonce::try_from(bytes.as_slice());
    assert_eq!(nonce.is_ok(), true);
    assert_eq!(nonce.as_ref().unwrap().size(), Nonce::BYTES);
    assert_eq!(nonce.as_ref().unwrap().as_bytes(), bytes);
}

#[test]
fn test_nonce_random() {
    let nonce = Nonce::random();
    assert_eq!(nonce.size(), Nonce::BYTES);
}

#[test]
fn test_nonce_increment() {
    let nonce1 = Nonce::random();
    let mut nonce2 = nonce1.clone();
    let nonce2 = nonce2.increment();
    assert_eq!(nonce1.size(), Nonce::BYTES);
    assert_eq!(nonce2.size(), Nonce::BYTES);
   // println!("nonce1:{} nonce2:{}", nonce1, nonce2);
}

#[test]
fn test_nonce_clear() {
    let bytes = create_random_bytes(Nonce::BYTES);
    let nonce = Nonce::try_from(bytes.as_slice());
    assert_eq!(nonce.is_ok(), true);
    let zero_bytes = vec![0u8; Nonce::BYTES];

    let mut nonce = nonce.unwrap();
    nonce.clear();
    assert_eq!(nonce.size(), Nonce::BYTES);
    assert_eq!(nonce.as_bytes(), zero_bytes);
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
fn test_keypair_tryfrom_bytes() {
     let bytes = create_random_bytes(KeyPair::SEED_BYTES);
     let kp = KeyPair::try_from(bytes.as_slice());
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
fn test_keypair_trait_tryfrom_signature_keypair() {
    let signature_kp = signature::KeyPair::random();
    let kp = KeyPair::from(&signature_kp);
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
# Cryptobox APIs.
- try_from(&PublicKey, &PrivateKey)
- size()
- as_bytes()
- clear()
- encrypt(plain, cipher, nonce) -> Result<usize>
- encrypt_into(plain, nonce) -> Result<Vec<u8>>
- decrypt(cipher, plain, nonce) -> Result<usize>
- decrypt_into(cipher, nonce) -> Result<Vec<u8>>
*/
#[test]
fn test_cryptbox_trait_from_skp() {
    let kp = KeyPair::random();
    let bx = CryptoBox::try_from((kp.public_key(),kp.private_key()));
    assert_eq!(bx.is_ok(), true);
    let bx = bx.unwrap();
    assert_eq!(bx.size(), CryptoBox::SYMMETRIC_KEY_BYTES);
}

#[test]
fn test_cryptbox_encryption() {
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let bx1 = CryptoBox::try_from((kp1.public_key(),kp2.private_key()));
    let bx2 = CryptoBox::try_from((kp2.public_key(),kp1.private_key()));
    assert_eq!(bx1.is_ok(), true);
    assert_eq!(bx2.is_ok(), true);

    let bx1 = bx1.unwrap();
    let nonce = Nonce::random();
    let plain = create_random_bytes(32);
    let mut cipher = vec![0u8; 1024];
    let result = bx1.encrypt(&plain, &mut cipher.as_mut_slice(), &nonce);
    assert_eq!(result.is_ok(), true);
    let cipher_len = result.unwrap();
    assert_eq!(cipher_len, plain.len() + CryptoBox::MAC_BYTES);

    let bx2 = bx2.unwrap();
    let mut decrypted = vec![0u8; 1024];
    let result = bx2.decrypt(&cipher[..cipher_len], &mut decrypted.as_mut_slice(), &nonce);
    assert_eq!(result.is_ok(), true);
    let decrypted_len = result.unwrap();
    assert_eq!(decrypted_len, cipher_len - CryptoBox::MAC_BYTES);
    assert_eq!(decrypted_len, plain.len());
    assert_eq!(decrypted[..decrypted_len], plain);
}

#[test]
fn test_cryptbox_encryption_into() {
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let bx1 = CryptoBox::try_from((kp1.public_key(),kp2.private_key()));
    let bx2 = CryptoBox::try_from((kp2.public_key(),kp1.private_key()));
    assert_eq!(bx1.is_ok(), true);
    assert_eq!(bx2.is_ok(), true);

    let bx = bx1.unwrap();
    let nonce = Nonce::random();
    let plain = create_random_bytes(32);
    let result = bx.encrypt_into(&plain, &nonce);
    assert_eq!(result.is_ok(), true);
    let cipher = result.unwrap();
    assert_eq!(cipher.len(), plain.len() + CryptoBox::MAC_BYTES);

    let bx = bx2.unwrap();
    let result = bx.decrypt_into(&cipher, &nonce);
    assert_eq!(result.is_ok(), true);
    let decrypted = result.unwrap();
    assert_eq!(decrypted.len(), cipher.len() - CryptoBox::MAC_BYTES);
    assert_eq!(decrypted.len(), plain.len());
    assert_eq!(plain, decrypted);
}

#[test]
fn test_cryptbox_clear() {
    let kp = KeyPair::random();
    let bx = CryptoBox::try_from((kp.public_key(),kp.private_key()));
    assert_eq!(bx.is_ok(), true);
    let mut bx = bx.unwrap();
    bx.clear();

    let zb = [0u8; CryptoBox::SYMMETRIC_KEY_BYTES];
    assert_eq!(bx.as_bytes(), zb);
}

/*
# Gobal APIs.
- encrypt(cipher, plain, nonce, pk, sk) -> Result<usize>
- encrypt_into(plain, nonce, pk, sk) -> Result<Vec<u8>>
- decrypt(plain, cipher, nonce, pk, sk) -> Result<usize>
- decrypt_into(cipher, nonce, pk, sk) -> Result<Vec<u8>>
*/
#[test]
fn test_encryption() {
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let nonce = Nonce::random();
    let plain = create_random_bytes(32);
    let mut cipher = vec![0u8; 1024];
    let result = boson::core::cryptobox::encrypt(
        &plain,
        &mut cipher.as_mut(),
        &nonce,
        kp1.public_key(),
        kp2.private_key()
    );
    assert_eq!(result.is_ok(), true);
    let cipher_len = result.unwrap();
    assert_eq!(cipher_len, plain.len() + CryptoBox::MAC_BYTES);

    let mut decrypted = vec![0u8; 1024];
    let result = boson::core::cryptobox::decrypt(
        &cipher[..cipher_len],
        &mut decrypted.as_mut(),
        &nonce,
        kp2.public_key(),
        kp1.private_key()
    );
    assert_eq!(result.is_ok(), true);
    let decrypted_len = result.unwrap();
    assert_eq!(decrypted_len, cipher_len - CryptoBox::MAC_BYTES);
    assert_eq!(decrypted_len, plain.len());
    assert_eq!(decrypted[..decrypted_len], plain);
}

#[test]
fn test_encryption_into() {
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let nonce = Nonce::random();
    let plain = create_random_bytes(32);
    let result = boson::core::cryptobox::encrypt_into(
        &plain,
        &nonce,
        kp1.public_key(),
        kp2.private_key()
    );
    assert_eq!(result.is_ok(), true);
    let cipher = result.unwrap();
    assert_eq!(cipher.len(), plain.len() + CryptoBox::MAC_BYTES);

    let result = boson::core::cryptobox::decrypt_into(
        &cipher,
        &nonce,
        kp2.public_key(),
        kp1.private_key()
    );
    assert_eq!(result.is_ok(), true);
    let decrypted = result.unwrap();
    assert_eq!(decrypted.len(), cipher.len() - CryptoBox::MAC_BYTES);
    assert_eq!(decrypted.len(), plain.len());
    assert_eq!(decrypted, plain);
}

// Try to test encryption with CryptoBox and decryption without it.
#[test]
fn test_cryptobox_crossways1() {
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let nonce = Nonce::random();
    let plain = create_random_bytes(32);
    let mut cipher = vec![0u8; 1024];
    let result = boson::core::cryptobox::encrypt(
        &plain,
        &mut cipher.as_mut(),
        &nonce,
        kp2.public_key(),
        kp1.private_key()
    );
    assert_eq!(result.is_ok(), true);
    let cipher_len = result.unwrap();
    assert_eq!(cipher_len, plain.len() + CryptoBox::MAC_BYTES);

    let result = CryptoBox::try_from((kp1.public_key(),kp2.private_key()));
    assert_eq!(result.is_ok(), true);
    let bx = result.unwrap();

    let mut decrypted = vec![0u8; 1024];
    let result = bx.decrypt(&cipher[..cipher_len], &mut decrypted.as_mut_slice(), &nonce);
    assert_eq!(result.is_ok(), true);
    let decrypted_len = result.unwrap();
    assert_eq!(decrypted_len, cipher_len - CryptoBox::MAC_BYTES);
    assert_eq!(decrypted_len, plain.len());
    assert_eq!(decrypted[..decrypted_len], plain);
}

#[test]
fn test_cryptobox_crossways2() {
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let nonce = Nonce::random();
    let plain = create_random_bytes(32);
    let mut cipher = vec![0u8; 1024];

    let result = CryptoBox::try_from((kp1.public_key(),kp2.private_key()));
    assert_eq!(result.is_ok(), true);
    let bx = result.unwrap();

    let result = bx.encrypt(&plain, &mut cipher.as_mut_slice(), &nonce);
    assert_eq!(result.is_ok(), true);
    let cipher_len = result.unwrap();
    assert_eq!(cipher_len, plain.len() + CryptoBox::MAC_BYTES);

    let mut decrypted = vec![0u8; 1024];
    let result = boson::core::cryptobox::decrypt(
        &cipher[..cipher_len],
        &mut decrypted.as_mut(),
        &nonce,
        kp2.public_key(),
        kp1.private_key()
    );
    assert_eq!(result.is_ok(), true);
    let decrypted_len = result.unwrap();
    assert_eq!(decrypted_len, cipher_len - CryptoBox::MAC_BYTES);
    assert_eq!(decrypted_len, plain.len());
    assert_eq!(decrypted[..decrypted_len], plain);
}
