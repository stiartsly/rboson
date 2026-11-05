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

use serde::{Serialize, Deserialize};
use serde_cbor::Value;

use boson_api::{
    CryptoIdentity,
    Id,
    Identity,
    Result,
    utils,
};
use crate::{
    contact::{Contact, ContactType},
    errors::{AuthenticationError, EncodingError}
};
use super::OpaqueContact;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContactData {
    #[serde(rename = "id")]
    pub(crate) id: Id,

    #[serde(rename = "t")]
    pub(crate) kind: u8,

    #[serde(rename = "sk", default, with = "utils::bytes_opt")]
    session_key: Option<Vec<u8>>,

    #[serde(
        rename = "n",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) name: Option<String>,

    #[serde(
        rename = "r",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) remark: Option<String>,

    #[serde(
        rename = "ts",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) tags: Option<String>,

    #[serde(
        rename = "m",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) muted: bool,

    #[serde(
        rename = "b",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) blocked: bool,

    #[serde(rename = "c")]
    pub(crate) created_at: i64,

    #[serde(rename = "u")]
    pub(crate) updated_at: i64,

    #[serde(rename = "v")]
    pub(crate) revision: i32,
}

impl ContactData {
    pub(crate) fn new(id: Id, kind: u8, created_at: i64) -> Self {
        Self {
            id,
            kind,
            created_at,
            updated_at: created_at,
            ..Default::default()
        }
    }

    pub(crate) fn set_blocked(&mut self, value: bool) {
        self.blocked = value;
    }

    pub(crate) fn set_updated(&mut self, updated_at: i64) {
        self.updated_at = updated_at;
    }

    pub(crate) fn set_revision(&mut self, revision: i32) {
        self.revision = revision;
    }

    pub(crate) fn set_session_key(&mut self, session_key: Vec<u8>) {
        self.session_key = Some(session_key);
    }

    /// Materialises a public [`Contact`] with the revision from the envelope.
    pub(crate) fn contact(&self, revision: i32) -> Result<Contact> {
        let mut data = self.clone();
        data.revision = revision;
        Contact::try_from(data)
    }

    /// Decrypts and parses the session identity, if this contact has one.
    pub(crate) fn session(&self, identity: &CryptoIdentity) -> Result<Option<CryptoIdentity>> {
        let Some(encrypted) = self.session_key.as_deref() else {
            return if self.kind == ContactType::Auto as u8 {
                Ok(None)
            } else {
                Err(EncodingError::new("Contact session key is missing or malformed"))
            };
        };

        let key = identity
            .decrypt_into(identity.id(), encrypted)
            .map_err(|error| {
                AuthenticationError::new(format!(
                    "Decrypting contact session key failed: {error}"
                ))
            })?;
        let session = CryptoIdentity::try_from(key.as_slice()).map_err(|error| {
            AuthenticationError::new(format!("Invalid contact session key: {error}"))
        })?;
        Ok(Some(session))
    }

    /// Encrypts this document into an opaque contact envelope.
    ///
    /// Mirrors [`OpaqueContact::decode_into`] in reverse: validate, encode,
    /// then encrypt.
    pub(crate) fn encode_into(&self, identity: &CryptoIdentity) -> Result<OpaqueContact> {
        // 1. Validate: the document assembles into a contact, and its session
        //    key, if any, is well-formed.
        self.contact(self.revision)?;
        self.session(identity)?;

        // 2. Encode: the document to CBOR bytes.
        let encoded = serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding contact failed: {error}")))?;

        // 3. Encrypt: the bytes for ourselves, so only we can read them back.
        let encrypted = identity
            .encrypt_into(identity.id(), &encoded)
            .map_err(|error| {
                AuthenticationError::new(format!("Encrypting contact failed: {error}"))
            })?;

        Ok(OpaqueContact::new(
            self.id,
            self.revision,
            Value::Bytes(encrypted),
        ))
    }
}

impl From<&Contact> for ContactData {
    fn from(contact: &Contact) -> Self {
        Self {
            id:     contact.id().clone(),
            kind:   contact.contact_type() as u8,
            session_key: None,
            name:   contact.name().map(|v|v.into()),
            remark: contact.remark().map(|v|v.into()),
            tags:   contact.tags().map(|v|v.into()),
            muted:  contact.is_muted(),
            blocked: contact.is_blocked(),
            created_at: contact.created_at(),
            updated_at: contact.updated_at(),
            revision: contact.revision(),
        }
    }
}

impl From<Contact> for ContactData {
    fn from(contact: Contact) -> Self {
        Self::from(&contact)
    }
}
