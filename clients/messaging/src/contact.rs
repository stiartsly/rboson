use boson_api::{
    Id,
    errors::{Result, Error, ArgumentError}
};
use serde::{Deserialize, Serialize};
use crate::{
    errors::EncodingError,
    contact_editor::{ContactEditor, ContactEditorOps},
    internal::{
        contact::ContactData
    },
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

/// A contact entry in the local contact list.
///
/// This trait mirrors the Java `Contact` interface and is implemented by both
/// individual contacts (friends) and channel contacts.
pub trait ContactOps: Send + Sync {
    /// The unique boson `Id` of this contact.
    fn id(&self) -> &Id;

    /// The type of this contact.
    fn contact_type(&self) -> ContactType;

    /// Display name, either the user-set remark or the contact's published name.
    fn name(&self) -> Option<&str>;

    /// User-defined remark (alias) for this contact.
    fn remark(&self) -> Option<&str>;

    /// User-defined tags attached to this contact.
    fn tags(&self) -> Option<&str>;

    /// Whether this contact has been muted.
    fn is_muted(&self) -> bool;

    /// Whether this contact has been blocked.
    fn is_blocked(&self) -> bool;

    /// Timestamp when this contact was first created locally, in milliseconds.
    fn created_at(&self) -> i64;

    /// Timestamp of the last update to this contact's record, in milliseconds.
    fn updated_at(&self) -> i64;

    /// Monotonically increasing revision counter for sync purposes.
    fn revision(&self) -> i32;

    /// Returns `true` if `other` refers to the same contact as `self`.
    fn is(&self, other: &dyn ContactOps) -> bool {
        self.id() == other.id()
    }

    /// Creates an editor that builds a new contact without mutating this one.
    fn edit(&self) -> ContactEditor;
}

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
#[serde(into = "ContactData", try_from = "ContactData")]
pub struct Contact {
    id: Id,
    contact_type: ContactType,
    name: Option<String>,
    remark: Option<String>,
    tags: Option<String>,
    muted: bool,
    blocked: bool,
    created_at: i64,
    updated_at: i64,
    revision: i32,
}

impl Contact {
    pub(crate) fn new(id: Id, contact_type: ContactType, created_at: i64) -> Self {
        Self {
            id, contact_type, name: None, remark: None, tags: None,
            muted: false, blocked: false, created_at, updated_at: created_at, revision: 0,
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

impl ContactEditorOps for ContactEditor {
    fn remark(mut self: Box<Self>, remark: Option<String>) -> Self {
        self.contact.remark = remark;
        *self
    }

    fn tags(mut self: Box<Self>, tags: Option<String>) -> Self {
        self.contact.tags = tags;
        *self
    }

    fn muted(mut self: Box<Self>, muted: bool) -> Self {
        self.contact.muted = muted;
        *self
    }

    fn blocked(mut self: Box<Self>, blocked: bool) -> Self {
        self.contact.blocked = blocked;
        *self
    }

    fn build(mut self: Box<Self>) -> Contact {
        self.contact.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.contact.revision += 1;
        self.contact
    }
}

impl ContactOps for Contact {
    fn id(&self) -> &Id {
        &self.id
    }

    fn contact_type(&self) -> ContactType {
        self.contact_type
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

    fn edit(&self) -> ContactEditor {
        ContactEditor { contact: self.clone() }
    }
}
