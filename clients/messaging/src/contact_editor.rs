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
