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

use boson_api::{
    utils,
    Id,
    Identity,
    Result,
    Error,
};
use crate::{
    Contact, ContactEditorHandler, ContactHandler, contact::ContactType, errors::{AuthenticationError, EncodingError}
};
use super::{
    ChannelInfo,
    contact_opaque::OpaqueContact,
};

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct PhotonContact {
     #[serde(rename = "id", with = "utils::id")]
    pub(crate) id: Id,

    #[serde(rename = "t")]
    pub(crate) contact_type: u8,

    #[serde(
        rename = "sk",
        default, with = "utils::bytes_opt"
    )]
    pub(crate) session_key: Option<Vec<u8>>,

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

    #[serde(skip)]
    pub(crate) channel: Option<crate::database::StoredChannel>,
}

impl PhotonContact {
    pub(crate) fn new(id: Id, contact_type: ContactType, created_at: i64) -> Self {
        Self {
            id,
            contact_type: contact_type as u8,
            session_key: None,
            created_at,
            updated_at: created_at,

            name: None,
            remark: None,
            tags: None,
            muted: false,
            blocked: false,
            revision: 0,
            channel: None,
        }
    }

    pub(crate) fn from(ci: &ChannelInfo, session_key: Option<Vec<u8>>) -> Self {
        Self {
            id: *ci.channel_id(),
            contact_type: ContactType::Channel as u8,
            session_key,
            name: ci.name().map(ToString::to_string),
            remark: None,
            tags: None,
            muted: false,
            blocked: false,
            created_at: ci.created_at(),
            updated_at: ci.updated_at(),
            revision: 0,
            channel: None,
        }
    }

    pub(crate) fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    pub(crate) fn set_session_key(&mut self, session_key: Option<Vec<u8>>) {
        self.session_key = session_key;
    }

    pub(crate) fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    pub(crate) fn set_remark(&mut self, remark: Option<String>) {
        self.remark = remark;
    }

    pub(crate) fn set_tags(&mut self, tags: Option<String>) {
        self.tags = tags;
    }

    pub(crate) fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub(crate) fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
    }

    pub(crate) fn set_revision(&mut self, revision: i32) {
        self.revision = revision;
    }

    pub(crate) fn set_updated_at(&mut self, updated_at: i64) {
        self.updated_at = updated_at;
    }

    pub fn is(&self, other: &Self) -> bool {
        self.id == other.id
    }

    pub(crate) fn session(&self, identity: &boson_api::CryptoIdentity) -> Result<Option<boson_api::CryptoIdentity>> {
        let Some(encrypted) = self.session_key.as_deref() else {
            return if self.contact_type == ContactType::Auto as u8 {
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
        let session = boson_api::CryptoIdentity::try_from(key.as_slice()).map_err(|error| {
            AuthenticationError::new(format!("Invalid contact session key: {error}"))
        })?;
        Ok(Some(session))
    }

    pub(crate) fn to_opaque(&self, data: Vec<u8>) -> OpaqueContact {
        OpaqueContact::new(
            self.id,
            self.revision,
            data
        )
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|e|
            EncodingError::new(format!("Serialization error: {e}")).into())
    }
}

impl PartialEq for PhotonContact {
    fn eq(&self, other: &Self) -> bool {
        self.is(other)
    }
}

impl PhotonContact {
    pub(crate) fn from_contact(contact: &crate::Contact) -> Self {
        Self {
            id: *contact.id(),
            contact_type: contact.contact_type() as u8,
            session_key: contact.session_key().map(|key| key.to_vec()),
            name: contact.name().map(ToString::to_string),
            remark: contact.remark().map(ToString::to_string),
            tags: contact.tags().map(ToString::to_string),
            muted: contact.is_muted(),
            blocked: contact.is_blocked(),
            created_at: contact.created_at(),
            updated_at: contact.updated_at(),
            revision: contact.revision(),
            channel: None,
        }
    }

    pub(crate) fn to_opaque_document(&self, identity: &boson_api::CryptoIdentity) -> Result<OpaqueContact> {
        let encoded = serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding contact failed: {error}")))?;
        let encrypted = identity
            .encrypt_into(identity.id(), &encoded)
            .map_err(|error| EncodingError::new(format!("Encrypting contact failed: {error}")))?;
        Ok(self.to_opaque(encrypted))
    }
}

impl ContactHandler for PhotonContact {
    fn id(&self) -> &Id {
        &self.id
    }

    fn contact_type(&self) -> ContactType {
        ContactType::from_i32(self.contact_type.into())
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn remark(&self) -> Option<&str> {
        self.remark.as_deref()
    }

    fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    fn is_muted(&self) -> bool {
        self.muted
    }

    fn is_blocked(&self) -> bool {
        self.blocked
    }

    fn created_at(&self) -> i64 {
        self.created_at
    }

    fn updated_at(&self) -> i64 {
        self.updated_at
    }

    fn revision(&self) -> i32 {
        self.revision
    }

    fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    fn edit(&self) -> Box<dyn ContactEditorHandler> {
        Box::new(PhotonContactEditor(self.clone())) as Box<dyn ContactEditorHandler>
    }

    fn clone_box(&self) -> Box<dyn ContactHandler> {
        Box::new(self.clone())
    }

    fn set_revision(&mut self, revision: i32) {
        self.revision = revision;
    }
}

impl Into<Contact> for PhotonContact {
    fn into(self) -> Contact {
        Box::new(self)
    }
}

impl From<Contact> for PhotonContact {
    fn from(contact: Contact) -> Self {
        Self::from_contact(&contact)
    }
}

pub struct PhotonContactEditor(PhotonContact);

impl ContactEditorHandler for PhotonContactEditor {
    fn set_name(mut self: Box<Self>, name: Option<String>) -> Box<dyn ContactEditorHandler> {
        self.0.name = name;
        self
    }

    fn set_remark(mut self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditorHandler> {
        self.0.remark = remark;
        self
    }

    fn set_tags(mut self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditorHandler> {
        self.0.tags = tags;
        self
    }

    fn set_muted(mut self: Box<Self>, muted: bool) -> Box<dyn ContactEditorHandler> {
        self.0.muted = muted;
        self
    }

    fn set_blocked(mut self: Box<Self>, blocked: bool) -> Box<dyn ContactEditorHandler> {
        self.0.blocked = blocked;
        self
    }

    fn set_session_key(mut self: Box<Self>, session_key: Option<Vec<u8>>) -> Box<dyn ContactEditorHandler> {
        self.0.session_key = session_key;
        self
    }

    fn set_updated_at(mut self: Box<Self>, updated_at: i64) -> Box<dyn ContactEditorHandler> {
        self.0.updated_at = updated_at;
        self
    }

    fn set_revision(mut self: Box<Self>, revision: i32) -> Box<dyn ContactEditorHandler> {
        self.0.revision = revision;
        self
    }

    fn build(mut self: Box<Self>) -> Box<dyn ContactHandler> {
        self.0.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.0.revision += 1;
        Box::new(self.0) as Box<dyn ContactHandler>
    }
}

#[cfg(test)]
#[path = "../unitests/test_contact.rs"]
mod test_contact;
