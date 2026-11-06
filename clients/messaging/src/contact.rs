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
    errors::ArgumentError,
};
use crate::{
    ContactEditorOps,
    ContactOps,
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

pub type Contact = Box<dyn ContactOps>;
pub type ContactEditor = Box<dyn ContactEditorOps>;
