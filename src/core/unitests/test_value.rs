use crate::core::{
    Id,
    Value,
    ValueBuilder,
    SignedBuilder,
    EncryptedBuilder,
    signature,
    cryptobox
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_immutable() {
        let data = crate::random_bytes(32);
        let rc = ValueBuilder::new(&data).build();
        assert_eq!(rc.is_ok(), true);

        let val: Value = rc.unwrap();
        assert_eq!(val.is_mutable(), false);
        assert_eq!(val.is_signed(), false);
        assert_eq!(val.is_encrypted(), false);
        assert_eq!(val.is_valid(), true);
        assert_eq!(val.public_key().is_some(), false);
        assert_eq!(val.private_key().is_some(), false);
        assert_eq!(val.recipient().is_some(), false);
        assert_eq!(val.signature().is_some(), false);
        assert_eq!(val.nonce().is_some(), false);
        assert_eq!(val.sequence_number(), 0);
        assert_eq!(val.data(), &data);

        let ser = serde_cbor::to_vec(&val).expect("Failed to serialize value");
        let des: Value = serde_cbor::from_slice(&ser).expect("Failed to deserialize value");
        assert_eq!(val, des);

        assert_eq!(des.is_mutable(), false);
        assert_eq!(des.is_signed(), false);
        assert_eq!(des.is_encrypted(), false);
        assert_eq!(des.is_valid(), true);
        assert_eq!(des.public_key().is_some(), false);
        assert_eq!(des.private_key().is_some(), false);
        assert_eq!(des.recipient().is_some(), false);
        assert_eq!(des.signature().is_some(), false);
        assert_eq!(des.nonce().is_some(), false);
        assert_eq!(des.sequence_number(), 0);
        assert_eq!(des.data(), &data);
    }

    #[test]
    fn test_signed() {
        let data = crate::random_bytes(32);
        let kp = signature::KeyPair::random();
        let nonce = cryptobox::Nonce::random();
        let seq = 55;
        let rc = SignedBuilder::new(&data)
            .with_keypair(&kp)
            .with_sequence_number(seq)
            .with_nonce(&nonce)
            .build();
        assert_eq!(rc.is_ok(), true);

        let val: Value = rc.unwrap();
        assert_eq!(val.is_mutable(), true);
        assert_eq!(val.is_signed(), true);
        assert_eq!(val.is_encrypted(), false);
        assert_eq!(val.is_valid(), true);
        assert_eq!(val.public_key().is_some(), true);
        assert_eq!(val.private_key().is_some(), true);
        assert_eq!(val.recipient().is_some(), false);
        assert_eq!(val.signature().is_some(), true);
        assert_eq!(val.nonce().is_some(), true);
        assert_eq!(val.sequence_number(), seq);
        assert_eq!(val.data(), &data);

        assert_eq!(val.public_key(), Some(kp.public_key().into()).as_ref());
        assert_eq!(val.private_key(), Some(kp.private_key()));
        assert_eq!(val.nonce(), Some(&nonce));

        let ser = serde_cbor::to_vec(&val).expect("Failed to serialize value");
        let des: Value = serde_cbor::from_slice(&ser).expect("Failed to deserialize value");

        assert_eq!(des.is_mutable(), true);
        assert_eq!(des.is_signed(), true);
        assert_eq!(des.is_encrypted(), false);
        assert_eq!(des.is_valid(), true);
        assert_eq!(des.public_key(), Some(kp.public_key().into()).as_ref());
        assert_eq!(des.private_key(), None);
        assert_eq!(des.recipient().is_none(), true);
        assert_eq!(des.signature(), val.signature());
        assert_eq!(des.nonce(), Some(&nonce));
        assert_eq!(des.sequence_number(), seq);
        assert_eq!(des.data(), &data);
    }

    #[test]
    fn test_encrypted() {
        let data = crate::random_bytes(32);
        let kp = signature::KeyPair::random();
        let nonce = cryptobox::Nonce::random();
        let rec: Id = signature::KeyPair::random()
            .public_key()
            .into();
        let rc = EncryptedBuilder::new(&data, &rec)
            .with_keypair(&kp)
            .with_nonce(&nonce)
            .with_sequence_number(55)
            .build();
        assert_eq!(rc.is_ok(), true);

        let val: Value = rc.unwrap();
        assert_eq!(val.is_mutable(), true);
        assert_eq!(val.is_signed(), true);
        assert_eq!(val.is_encrypted(), true);
        assert_eq!(val.is_valid(), true);
        assert_eq!(val.public_key().is_some(), true);
        assert_eq!(val.private_key().is_some(), true);
        assert_eq!(val.recipient().is_some(), true);
        assert_eq!(val.nonce().is_some(), true);
        assert_eq!(val.signature().is_some(), true);
        assert_eq!(val.sequence_number(), 55);
        assert_ne!(val.data(), &data);
        assert_eq!(val.recipient(), Some(rec).as_ref());
        assert_eq!(val.nonce(), Some(&nonce));
        assert_eq!(val.public_key(), Some(&kp.public_key().into()));
        assert_eq!(val.private_key(), Some(kp.private_key()));

        let ser = serde_cbor::to_vec(&val).expect("Failed to serialize value");
        let des: Value = serde_cbor::from_slice(&ser).expect("Failed to deserialize value");

        assert_eq!(des.is_mutable(), true);
        assert_eq!(des.is_signed(), true);
        assert_eq!(des.is_encrypted(), true);
        assert_eq!(des.is_valid(), true);
        assert_eq!(des.public_key(), Some(&kp.public_key().into()));
        assert_eq!(des.private_key(), None);
        assert_eq!(des.recipient(), Some(rec).as_ref());
        assert_eq!(des.nonce(), Some(&nonce));
        assert_eq!(des.signature(), val.signature());
        assert_eq!(des.sequence_number(), 55);
        assert_eq!(des.data(), val.data());
    }
}
