// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use boson_api::{CryptoIdentity, Id, Identity, Result};
use crate::errors::{AuthenticationError, EncodingError};
use super::ContactData;

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

    pub(crate) fn id(&self) -> &Id {
        &self.id
    }

    pub(crate) fn revision(&self) -> i32 {
        self.revision
    }

    /// Decrypts the envelope and validates the contact document inside it.
    pub(crate) fn decode_into(&self, identity: &CryptoIdentity) -> Result<(ContactData, Vec<u8>)> {
        if self.revision < 0 {
            return Err(EncodingError::new(format!(
                "Contact revision is negative {}",
                self.revision
            )));
        }
        let Value::Bytes(encrypted) = &self.data else {
            return Err(EncodingError::new("Opaque contact data is not binary"));
        };

        // 1. Decrypt: the envelope that was sealed for ourselves.
        let encoded = identity
            .decrypt_into(identity.id(), encrypted)
            .map_err(|error| AuthenticationError::new(format!("Decrypting contact failed: {error}")))?;

        // 2. Decode: the CBOR bytes back into a document.
        let Value::Map(mut decoded) = serde_cbor::from_slice::<Value>(&encoded)
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?
        else {
            return Err(EncodingError::new("Contact data is not a map"));
        };
        // Older envelopes may omit the revision in the payload; the envelope
        // revision is authoritative for the resulting public contact.
        decoded
            .entry(Value::Text("v".into()))
            .or_insert(Value::Integer(self.revision.into()));

        let data: ContactData = serde_cbor::value::from_value(Value::Map(decoded))
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?;

        if data.id != self.id {
            return Err(EncodingError::new("Contact ID mismatch"));
        }
        data.contact(self.revision)?;

        Ok((data, encoded))
    }
}
