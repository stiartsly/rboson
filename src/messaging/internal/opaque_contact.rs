use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use crate::{CryptoIdentity, Id, Identity, errors::Result};
use super::contact::ContactData;
use crate::messaging::errors::{AuthenticationError, EncodingError};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpaqueContact {
    #[serde(rename = "id")]
    id: Id,

    #[serde(rename = "v")]
    revision: i32,

    #[serde(rename = "d")]
    data: Value,
}

impl OpaqueContact {
    pub(crate) fn new(id: Id, revision: i32, data: Value) -> Self {
        Self { id, revision, data }
    }

    pub(crate) fn revision(&self) -> i32 {
        self.revision
    }

    pub(crate) fn decode(&self, identity: &CryptoIdentity) -> Result<(ContactData, Vec<u8>)> {
        if self.revision < 0 {
            return Err(EncodingError::new("Contact revision is negative"));
        }
        let Value::Bytes(encrypted) = &self.data else {
            return Err(EncodingError::new("Opaque contact data is not binary"));
        };
        let plaintext = identity.decrypt_into(identity.id(), encrypted)
            .map_err(|error| AuthenticationError::new(format!("Decrypting contact failed: {error}")))?;
        let Value::Map(mut data) = serde_cbor::from_slice::<Value>(&plaintext)
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))? else {
            return Err(EncodingError::new("Contact data is not a map"));
        };
        data.entry(Value::Text("v".into())).or_insert(Value::Integer(self.revision.into()));
        if let Some(Value::Bytes(bytes)) = data.get(&Value::Text("sk".into())) {
            let key = Value::Array(bytes.iter().map(|byte| Value::Integer((*byte).into())).collect());
            data.insert(Value::Text("sk".into()), key);
        }
        let data: ContactData = serde_cbor::value::from_value(Value::Map(data))
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?;
        if data.id != self.id {
            return Err(EncodingError::new("Contact ID mismatch"));
        }

        data.contact(self.revision)?;
        Ok((data, plaintext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::contact::ContactOps;

    #[test]
    fn opaque_contact_roundtrip_preserves_binary_session_key() {
        let user = CryptoIdentity::new();
        let session = CryptoIdentity::new();
        let mut data = ContactData::new(Id::random(), 1, 100);
        data.set_session_key(user.encrypt_into(
            user.id(), session.signature_keypair().private_key().as_ref(),
        ).unwrap());
        let opaque = data.to_opaque(7, &user).unwrap();
        let (decoded, plaintext) = opaque.decode(&user).unwrap();
        assert_eq!(decoded.contact(7).unwrap().revision(), 7);
        assert_eq!(decoded.session(&user).unwrap().unwrap().id(), session.id());
        let Value::Map(wire) = serde_cbor::from_slice::<Value>(&plaintext).unwrap() else {
            panic!("Contact data must be a map");
        };
        assert!(matches!(wire.get(&Value::Text("sk".into())), Some(Value::Bytes(_))));
    }

    #[test]
    fn opaque_contact_uses_envelope_revision_when_data_has_none() {
        let user = CryptoIdentity::new();
        let data = ContactData::new(Id::random(), 0, 100);
        let Value::Map(mut wire) = serde_cbor::value::to_value(&data).unwrap() else {
            panic!("Contact data must be a map");
        };
        wire.remove(&Value::Text("v".into()));
        let plaintext = serde_cbor::to_vec(&Value::Map(wire)).unwrap();
        let opaque = OpaqueContact::new(data.id, 9, Value::Bytes(
            user.encrypt_into(user.id(), &plaintext).unwrap(),
        ));
        let (decoded, _) = opaque.decode(&user).unwrap();
        assert_eq!(decoded.contact(opaque.revision()).unwrap().revision(), 9);
        assert!(decoded.session(&user).unwrap().is_none());
    }
}