use crate::core::{
    Id,
    cryptobox::{self, CryptoBox, Nonce},
    signature,
    CryptoContext,
};
/*
 Testcases for critical methods:
 - new(..)
 - from_private_key(..)
 */


#[cfg(test)]
mod tests {
    use super::*;

    fn create_contexts_from_private_key() -> (CryptoContext, CryptoContext) {
        let sig_kp1 = signature::KeyPair::random();
        let sig_kp2 = signature::KeyPair::random();
        let box_kp1 = cryptobox::KeyPair::from(&sig_kp1);
        let box_kp2 = cryptobox::KeyPair::from(&sig_kp2);

        let id1 = Id::from(sig_kp1.public_key());
        let id2 = Id::from(sig_kp2.public_key());

        let ctx1 = CryptoContext::from_private_key(id2.clone(), box_kp1.private_key());
        let ctx2 = CryptoContext::from_private_key(id1, box_kp2.private_key());

        (ctx1, ctx2)
    }

    fn create_contexts_from_cryptobox() -> (CryptoContext, CryptoContext) {
        let sig_kp1 = signature::KeyPair::random();
        let sig_kp2 = signature::KeyPair::random();
        let box_kp1 = cryptobox::KeyPair::from(&sig_kp1);
        let box_kp2 = cryptobox::KeyPair::from(&sig_kp2);

        let id1 = Id::from(sig_kp1.public_key());
        let id2 = Id::from(sig_kp2.public_key());

        let box1 = cryptobox::CryptoBox::try_from((box_kp2.public_key(), box_kp1.private_key())).unwrap();
        let box2 = cryptobox::CryptoBox::try_from((box_kp1.public_key(), box_kp2.private_key())).unwrap();
        let ctx1 = CryptoContext::new(id2.clone(), box1);
        let ctx2 = CryptoContext::new(id1, box2);

        (ctx1, ctx2)
    }

    #[test]
    fn test_from_sk() {
        let (ctx1, mut ctx2) = create_contexts_from_private_key();
        let plain = b"Hello, World!";

        assert_ne!(ctx1.id(), ctx2.id());

        // testing encrypt_into and decrypt_into methods
        let result = ctx2.encrypt_into(plain);
        assert!(result.is_ok());

        let cipher = result.unwrap();
        let result = ctx1.decrypt_into(&cipher);
        assert!(result.is_ok());

        let decrypted = result.unwrap();
        assert_eq!(plain, decrypted.as_slice());

        // testing encrypt and decrypt methods
        let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        let result = ctx2.encrypt(plain, &mut cipher);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), cipher.len());

        let result = ctx1.decrypt_into(&cipher);
        assert!(result.is_ok());

        let decrypted = result.unwrap();
        assert_eq!(plain, decrypted.as_slice());

        let mut decrypted = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        let result = ctx1.decrypt(&cipher, &mut decrypted);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), decrypted.len());
        assert_eq!(plain, decrypted.as_slice());
    }

    #[test]
    fn test_from_cryptobox() {
        let (ctx1, mut ctx2) = create_contexts_from_cryptobox();
        let plain = b"Hello, World!";

        assert_ne!(ctx1.id(), ctx2.id());

        // testing encrypt_into and decrypt_into methods
        let result = ctx2.encrypt_into(plain);
        assert!(result.is_ok());

        let cipher = result.unwrap();
        let result = ctx1.decrypt_into(&cipher);
        assert!(result.is_ok());

        let decrypted = result.unwrap();
        assert_eq!(plain, decrypted.as_slice());

        // testing encrypt and decrypt methods
        let mut cipher = vec![0u8; plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES];
        let result = ctx2.encrypt(plain, &mut cipher);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), cipher.len());

        let result = ctx1.decrypt_into(&cipher);
        assert!(result.is_ok());

        let decrypted = result.unwrap();
        assert_eq!(plain, decrypted.as_slice());

        let mut decrypted = vec![0u8; cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES];
        let result = ctx1.decrypt(&cipher, &mut decrypted);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), decrypted.len());
        assert_eq!(plain, decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_tampered_ciphertext() {
        let (ctx1, mut ctx2) = create_contexts_from_private_key();
        let plain = b"Hello, World!";
        let mut cipher = ctx2.encrypt_into(plain).unwrap();
        let last = cipher.len() - 1;
        cipher[last] ^= 0x01;

        let result = ctx1.decrypt_into(&cipher);
        assert!(result.is_err());
    }
}
