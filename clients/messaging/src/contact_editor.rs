use crate::contact::{
    Contact,
};

/// Builder for an immutable contact update.
pub trait ContactEditorOps: Send {
    /// Set the user-defined remark (alias).
    fn remark(self: Box<Self>, remark: Option<String>) -> Self;

    /// Set the user-defined tags.
    fn tags(self: Box<Self>, tags: Option<String>) -> Self;

    /// Toggle the muted state.
    fn muted(self: Box<Self>, muted: bool) -> Self;

    /// Toggle the blocked state.
    fn blocked(self: Box<Self>, blocked: bool) -> Self;

    /// Build the updated immutable contact.
    fn build(self: Box<Self>) -> Contact;
}


/// Builder for updating a [`Contact`].
pub struct ContactEditor {
    pub(crate) contact: Contact,
}
