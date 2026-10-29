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
use boson_api::{
    Id,
    errors::{Result, Error, ArgumentError}
};
use crate::{
    errors::EncodingError,
    contact_editor::ContactEditor,
    ops::ContactOps,
    internal::contact::ContactData
};

/// The type of a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContactType {
    /// Automatically added contact (e.g. channel member discovered via message).
    Auto = 0,
    /// Manually added friend contact.
    Friend = 1,
    /// A channel (group) contact.
    Channel = 2,
}

impl TryFrom<i32> for ContactType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(ContactType::Auto),
            1 => Ok(ContactType::Friend),
            2 => Ok(ContactType::Channel),
            _ => Err(ArgumentError::new("invalid contact type")),
        }
    }
}

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
#[serde(into = "ContactData", try_from = "ContactData")]
pub struct Contact {
    pub(crate) id          : Id,
    pub(crate) contact_type: ContactType,
    pub(crate) name        : Option<String>,
    pub(crate) remark      : Option<String>,
    pub(crate) tags        : Option<String>,
    pub(crate) muted       : bool,
    pub(crate) blocked     : bool,
    pub(crate) created_at  : i64,
    pub(crate) updated_at  : i64,
    pub(crate) revision    : i32,
}

impl Contact {
    pub(crate) fn new(id: Id, contact_type: ContactType, created_at: i64) -> Self {
        Self {
            id,
            contact_type,
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

impl PartialEq for Contact {
    fn eq(&self, other: &Self) -> bool {
        self.is(other)
    }
}

impl TryFrom<ContactData> for Contact {
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
            id: data.id, contact_type,
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

impl Contact {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn contact_type(&self) -> ContactType {
        self.contact_type
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn remark(&self) -> Option<&str> {
        self.remark.as_deref()
    }

    pub fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    pub fn revision(&self) -> i32 {
        self.revision
    }

    pub fn edit(&self) -> ContactEditor {
        ContactEditor { contact: self.clone() }
    }
}

impl ContactOps for Contact {
    fn id(&self) -> &Id {
        self.id()
    }

    fn contact_type(&self) -> ContactType {
        self.contact_type()
    }

    fn name(&self) -> Option<&str> {
        self.name()
    }

    fn remark(&self) -> Option<&str> {
        self.remark()
    }

    fn tags(&self) -> Option<&str> {
        self.tags()
    }

    fn is_muted(&self) -> bool {
        self.is_muted()
    }

    fn is_blocked(&self) -> bool {
        self.is_blocked()
    }

    fn created_at(&self) -> i64 {
        self.created_at()
    }

    fn updated_at(&self) -> i64 {
        self.updated_at()
    }

    fn revision(&self) -> i32 {
        self.revision()
    }

    fn edit(&self) -> ContactEditor {
        self.edit()
    }
}

#[cfg(test)]
#[path = "unitests/test_contact.rs"]
mod test_contact;
