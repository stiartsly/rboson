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

use crate::contact::Contact;
use crate::ops::ContactEditorOps;

pub struct ContactEditor {
    pub(crate) contact: Contact,
}

impl ContactEditor {
    pub fn remark(mut self: Box<Self>, remark: Option<String>) -> Self {
        self.contact.remark = remark;
        *self
    }

    pub fn tags(mut self: Box<Self>, tags: Option<String>) -> Self {
        self.contact.tags = tags;
        *self
    }

    pub fn muted(mut self: Box<Self>, muted: bool) -> Self {
        self.contact.muted = muted;
        *self
    }

    pub fn blocked(mut self: Box<Self>, blocked: bool) -> Self {
        self.contact.blocked = blocked;
        *self
    }

    pub fn build(mut self: Box<Self>) -> Contact {
        self.contact.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.contact.revision += 1;
        self.contact
    }
}

impl ContactEditorOps for ContactEditor {
    fn remark(self: Box<Self>, remark: Option<String>) -> Self {
        self.remark(remark)
    }

    fn tags(self: Box<Self>, tags: Option<String>) -> Self {
        self.tags(tags)
    }

    fn muted(self: Box<Self>, muted: bool) -> Self {
        self.muted(muted)
    }

    fn blocked(self: Box<Self>, blocked: bool) -> Self {
        self.blocked(blocked)
    }

    fn build(self: Box<Self>) -> Contact {
        self.build()
    }
}
