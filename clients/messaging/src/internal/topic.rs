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

use std::{
    fmt,
    str::FromStr
};

use boson_api::{
    Error,
    Result,
    errors::ArgumentError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Topic {
    UserInbox,
    UserOutbox,
    DeviceInbox,
    DeviceOutbox,
}

impl Topic {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UserInbox => "u/i",
            Self::UserOutbox => "u/o",
            Self::DeviceInbox => "d/i",
            Self::DeviceOutbox => "d/o",
        }
    }
}

impl AsRef<str> for Topic {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for Topic {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "u/i" => Ok(Self::UserInbox),
            "u/o" => Ok(Self::UserOutbox),
            "d/i" => Ok(Self::DeviceInbox),
            "d/o" => Ok(Self::DeviceOutbox),
            _ => Err(ArgumentError::new(format!("Invalid topic {}", value)))
        }
    }
}

impl TryFrom<&str> for Topic {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        value.parse()
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}
