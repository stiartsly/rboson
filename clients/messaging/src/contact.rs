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

use boson_api::{
    Result,
    Error,
    Id,
    errors::ArgumentError,
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

impl ContactType {
    pub fn from_i32(value: i32) -> Self {
        Self::try_from(value).unwrap_or(Self::Auto)
    }
}

impl TryFrom<i32> for ContactType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(ContactType::Auto),
            1 => Ok(ContactType::Friend),
            2 => Ok(ContactType::Channel),
            _ => Err(ArgumentError::new(format!("invalid contact type {}", value))),
        }
    }
}

/// A contact entry in the local contact list.
///
pub trait ContactHandler: Send + Sync {
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

    fn session_key(&self) -> Option<&[u8]> {
        None
    }

    /// Creates an editor that builds a new contact without mutating this one.
    fn edit(&self) -> Box<dyn ContactEditorHandler>;

    /// Clones this contact into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn ContactHandler>;

    /// Updates the local revision counter.
    fn set_revision(&mut self, revision: i32);
}

impl Clone for Box<dyn ContactHandler> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// Builder for an immutable contact update.
pub trait ContactEditorHandler: Send + Sync {
    /// Set the display name.
    fn set_name(self: Box<Self>, name: Option<String>) -> Box<dyn ContactEditorHandler>;

    /// Set the user-defined remark (alias).
    fn set_remark(self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditorHandler>;

    /// Set the user-defined tags.
    fn set_tags(self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditorHandler>;

    /// Toggle the muted state.
    fn set_muted(self: Box<Self>, muted: bool) -> Box<dyn ContactEditorHandler>;

    /// Toggle the blocked state.
    fn set_blocked(self: Box<Self>, blocked: bool) -> Box<dyn ContactEditorHandler>;

    /// Set the encrypted session key.
    fn set_session_key(self: Box<Self>, session_key: Option<Vec<u8>>) -> Box<dyn ContactEditorHandler>;

    /// Set the updated timestamp.
    fn set_updated_at(self: Box<Self>, updated_at: i64) -> Box<dyn ContactEditorHandler>;

    fn set_revision(self: Box<Self>, revision: i32) -> Box<dyn ContactEditorHandler>;

    /// Build the updated immutable contact.
    fn build(self: Box<Self>) -> Box<dyn ContactHandler>;
}

pub type Contact = Box<dyn ContactHandler>;
pub type ContactEditor = Box<dyn ContactEditorHandler>;
