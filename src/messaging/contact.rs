use crate::{
    Id,
    errors::{Result, Error, ArgumentError}
};
use serde::{Deserialize, Serialize};
use crate::messaging::{
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


#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    fn data(kind: u8, revision: i32) -> ContactData {
        let mut data = ContactData::new(Id::random(), kind, 100);
        data.name = Some("Alice".into());
        data.remark = Some("friend".into());
        data.tags = Some("test".into());
        data.muted = true;
        data.blocked = true;
        data.updated_at = 200;
        data.revision = revision;
        data
    }

    #[test]
    fn contact_roundtrip_uses_contact_data() {
        for kind in 0..=2 {
            let data = data(kind, 7);
            let contact = Contact::try_from(data.clone()).unwrap();
            let bytes = serde_cbor::to_vec(&contact).unwrap();
            assert_eq!(
                serde_cbor::from_slice::<Value>(&bytes).unwrap(),
                serde_cbor::value::to_value(&data).unwrap(),
            );
            let decoded: Contact = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded.id(), contact.id());
            assert_eq!(decoded.contact_type(), contact.contact_type());
            assert_eq!(decoded.name(), contact.name());
            assert_eq!(decoded.remark(), contact.remark());
            assert_eq!(decoded.tags(), contact.tags());
            assert_eq!(decoded.is_muted(), contact.is_muted());
            assert_eq!(decoded.is_blocked(), contact.is_blocked());
            assert_eq!(decoded.created_at(), contact.created_at());
            assert_eq!(decoded.updated_at(), contact.updated_at());
            assert_eq!(decoded.revision(), contact.revision());
        }
    }

    #[test]
    fn contact_deserialization_rejects_invalid_data() {
        let mut invalid = vec![data(3, 0), data(1, -1)];
        let mut negative_timestamp = data(1, 0);
        negative_timestamp.created_at = -1;
        invalid.push(negative_timestamp);
        let mut earlier_update = data(1, 0);
        earlier_update.updated_at = earlier_update.created_at - 1;
        invalid.push(earlier_update);
        for data in invalid {
            assert!(Contact::try_from(data.clone()).is_err());
            let bytes = serde_cbor::to_vec(&data).unwrap();
            assert!(serde_cbor::from_slice::<Contact>(&bytes).is_err());
        }
    }

    #[test]
    fn contact_data_preserves_envelope_revision() {
        let data = data(1, 0);
        let wire = serde_cbor::value::to_value(&data).unwrap();
        let Value::Map(wire) = wire else { panic!("Contact data must be a map") };
        assert_eq!(wire.get(&Value::Text("v".into())), Some(&Value::Integer(0.into())));
        let decoded: Contact = serde_cbor::value::from_value(Value::Map(wire)).unwrap();
        assert_eq!(decoded.revision(), 0);
        assert_eq!(data.contact(9).unwrap().revision(), 9);
        assert!(data.contact(-1).is_err());
    }

    #[test]
    fn contact_editor_preserves_original_contact() {
        let contact = Contact::try_from(data(1, 0)).unwrap();
        let editor = Box::new(contact.edit()).remark(Some("updated".into()));
        let editor = Box::new(editor).tags(None);
        let editor = Box::new(editor).muted(false);
        let editor = Box::new(editor).blocked(false);
        let edited = Box::new(editor).build();
        assert_eq!(edited.id(), contact.id());
        assert_eq!(edited.remark(), Some("updated"));
        assert_eq!(edited.tags(), None);
        assert!(!edited.is_muted());
        assert!(!edited.is_blocked());
        assert_eq!(edited.revision(), 1);
        assert_eq!(contact.remark(), Some("friend"));
        assert_eq!(contact.tags(), Some("test"));
        assert!(contact.is_muted());
        assert!(contact.is_blocked());
        assert_eq!(contact.revision(), 0);
    }
}
