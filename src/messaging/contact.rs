use std::time::SystemTime;
use crate::Id;

/// The type of a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContactType {
    /// Automatically added contact (e.g. channel member discovered via message).
    Auto    = 0,
    /// Manually added friend contact.
    Friend  = 1,
    /// A channel (group) contact.
    Channel = 2,
}

/// A contact entry in the local contact list.
///
/// This trait mirrors the Java `Contact` interface and is implemented by both
/// individual contacts (friends) and channel contacts.
pub trait Contact: Send + Sync {
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

    /// Timestamp when this contact was first created locally.
    fn created_at(&self) -> SystemTime;

    /// Timestamp of the last update to this contact's record.
    fn updated_at(&self) -> SystemTime;

    /// Monotonically increasing revision counter for sync purposes.
    fn revision(&self) -> i32;

    /// Optional avatar URI / identifier string.
    fn avatar(&self) -> Option<&str>;

    /// Returns `true` if this contact has an avatar set.
    fn has_avatar(&self) -> bool {
        self.avatar().is_some()
    }

    /// The name to show in the UI: remark if set, otherwise `name()`.
    fn display_name(&self) -> &str;

    /// Returns `true` if `other` refers to the same contact as `self`.
    fn is(&self, other: &dyn Contact) -> bool {
        self.id() == other.id()
    }
}

/// Mutable operations exposed by a contact when editing.
pub trait ContactEditor {
    /// Set the user-defined remark (alias).
    fn set_remark(&mut self, remark: Option<String>);

    /// Set the user-defined tags.
    fn set_tags(&mut self, tags: Option<String>);

    /// Toggle the muted state.
    fn set_muted(&mut self, muted: bool);

    /// Toggle the blocked state.
    fn set_blocked(&mut self, blocked: bool);
}
