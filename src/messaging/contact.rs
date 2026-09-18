use crate::Id;

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

    /// Timestamp when this contact was first created locally, in milliseconds.
    fn created_at(&self) -> i64;

    /// Timestamp of the last update to this contact's record, in milliseconds.
    fn updated_at(&self) -> i64;

    /// Monotonically increasing revision counter for sync purposes.
    fn revision(&self) -> i32;

    /// Returns `true` if `other` refers to the same contact as `self`.
    fn is(&self, other: &dyn Contact) -> bool {
        self.id() == other.id()
    }

    /// Creates an editor that builds a new contact without mutating this one.
    fn edit(&self) -> Box<dyn ContactEditor>;
}

/// Builder for an immutable contact update.
pub trait ContactEditor: Send {
    /// Set the user-defined remark (alias).
    fn remark(self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditor>;

    /// Set the user-defined tags.
    fn tags(self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditor>;

    /// Toggle the muted state.
    fn muted(self: Box<Self>, muted: bool) -> Box<dyn ContactEditor>;

    /// Toggle the blocked state.
    fn blocked(self: Box<Self>, blocked: bool) -> Box<dyn ContactEditor>;

    /// Build the updated immutable contact.
    fn build(self: Box<Self>) -> Box<dyn Contact>;
}
