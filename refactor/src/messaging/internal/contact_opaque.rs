use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use crate::messaging::errors::{AuthenticationError, EncodingError};
use crate::{CryptoIdentity, Id, Identity, Result};

use super::ContactData;

/// Encrypted contact document plus the revision of its envelope.
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

    /// Decrypts the envelope and validates the contact document inside it.
    ///
    /// Returns the document together with its plaintext so callers that need
    /// additional (non-contact) fields do not have to decrypt it again.
    pub(crate) fn decode(&self, identity: &CryptoIdentity) -> Result<(ContactData, Vec<u8>)> {
        if self.revision < 0 {
            return Err(EncodingError::new("Contact revision is negative"));
        }
        let Value::Bytes(encrypted) = &self.data else {
            return Err(EncodingError::new("Opaque contact data is not binary"));
        };

        let plaintext = identity
            .decrypt_into(identity.id(), encrypted)
            .map_err(|error| {
                AuthenticationError::new(format!("Decrypting contact failed: {error}"))
            })?;
        let Value::Map(mut document) = serde_cbor::from_slice::<Value>(&plaintext)
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?
        else {
            return Err(EncodingError::new("Contact data is not a map"));
        };

        // Older envelopes may omit the revision in the payload; the envelope
        // revision is authoritative for the resulting public contact.
        document
            .entry(Value::Text("v".into()))
            .or_insert(Value::Integer(self.revision.into()));

        let data: ContactData = serde_cbor::value::from_value(Value::Map(document))
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?;
        if data.id != self.id {
            return Err(EncodingError::new("Contact ID mismatch"));
        }

        data.contact(self.revision)?;
        Ok((data, plaintext))
    }
}
