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

use serde::{Serialize, Deserialize};
use crate::{
    Contact
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContactsUpdate {
    #[serde(rename = "v")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    #[allow(dead_code)]
    version_id: Option<String>,

    #[serde(rename = "c")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    contacts: Option<Vec<Contact>>,
}

impl ContactsUpdate {
    pub(crate) fn new(version_id: Option<String>, contacts: Vec<Contact>) -> Self {
        Self {
            version_id,
            contacts: Some(contacts)
        }
    }

    pub(crate) fn version_id(&mut self) -> Option<String> {
        //self.version_id.take()
        Some("1.1".to_string())
    }

    pub(crate) fn contacts(&mut self) -> Vec<Contact> {
        self.contacts.take().unwrap_or_default()
    }
}
