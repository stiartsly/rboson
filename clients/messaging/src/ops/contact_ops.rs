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

use boson_api::Id;
use crate::{
    contact::{Contact, ContactType},
    contact_editor::ContactEditor,
};

/// A contact entry in the local contact list.
///
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

// Builder for an immutable contact update.
pub trait ContactEditorOps: Send + Sync {
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
