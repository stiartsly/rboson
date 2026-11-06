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
    Result,
    Error,
};
use crate::{
    errors::EncodingError,
    ContactOps,
    internal::ContactData,
    contact::ContactType,
    ContactEditorOps,
};


#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct PhotonContact {
     #[serde(rename = "id", with = "utils::id")]
    pub(crate) id: Id,

    #[serde(rename = "t")]
    pub(crate) contact_type: u8,

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
        }
    }

    pub(crate) fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    pub(crate) fn set_name(&mut self, name: Option<String>) {
        self.name = name;
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
}

impl PartialEq for PhotonContact {
    fn eq(&self, other: &Self) -> bool {
        self.is(other)
    }
}

impl TryFrom<ContactData> for PhotonContact {
    type Error = Error;

    fn try_from(data: ContactData) -> Result<Self> {
        let contact_type = match data.kind {
            0 => ContactType::Auto,
            1 => ContactType::Friend,
            2 => ContactType::Channel,
            _ => return Err(EncodingError::new("Unknown contact type")),
        };

        if data.created_at < 0 ||
            data.updated_at < data.created_at ||
            data.revision < 0 {
            return Err(EncodingError::new("Invalid contact timestamps or revision"));
        }

        Ok(Self {
            id: data.id,
            contact_type: contact_type as u8,
            session_key: data.session_key().map(|key| key.to_vec()),
            name: data.name,
            remark: data.remark,
            tags: data.tags,
            muted: data.muted,
            blocked: data.blocked,
            created_at: data.created_at,
            updated_at: data.updated_at,
            revision: data.revision,
        })
    }
}

impl ContactOps for PhotonContact {
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

    fn edit(&self) -> Box<dyn ContactEditorOps> {
        Box::new(PhotonContactEditor(self.clone())) as Box<dyn ContactEditorOps>
    }

    fn clone_box(&self) -> Box<dyn ContactOps> {
        Box::new(self.clone())
    }

    fn set_revision(&mut self, revision: i32) {
        self.revision = revision;
    }
}

pub struct PhotonContactEditor(PhotonContact);

impl ContactEditorOps for PhotonContactEditor {
    fn remark(mut self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditorOps> {
        self.0.remark = remark;
        self
    }

    fn tags(mut self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditorOps> {
        self.0.tags = tags;
        self
    }

    fn muted(mut self: Box<Self>, muted: bool) -> Box<dyn ContactEditorOps> {
        self.0.muted = muted;
        self
    }

    fn blocked(mut self: Box<Self>, blocked: bool) -> Box<dyn ContactEditorOps> {
        self.0.blocked = blocked;
        self
    }

    fn build(mut self: Box<Self>) -> Box<dyn ContactOps> {
        self.0.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.0.revision += 1;
        Box::new(self.0) as Box<dyn ContactOps>
    }
}

#[cfg(test)]
#[path = "../unitests/test_contact.rs"]
mod test_contact;
