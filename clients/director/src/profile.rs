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
use serde::Deserialize;
use std::fmt;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Deserialize)]
pub struct Profile {
    #[serde(rename = "id")]
    id: Id,
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    bio: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: u64,
    #[serde(rename = "updatedAt")]
    updated_at: u64,
    #[serde(rename = "planName")]
    plan_name: String,
    #[serde(rename = "passphraseProtected")]
    passphrase_protected: bool,
}

impl Profile {
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn admin(&self) -> bool {
        self.admin
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn avatar(&self) -> Option<&String> {
        self.avatar.as_ref()
    }

    pub fn email(&self) -> Option<&String> {
        self.email.as_ref()
    }

    pub fn bio(&self) -> Option<&String> {
        self.bio.as_ref()
    }

    pub fn created_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.created_at)
    }

    pub fn updated_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.updated_at)
    }

    pub fn plan_name(&self) -> &String {
        &self.plan_name
    }

    pub fn is_passphrase_protected(&self) -> bool {
        self.passphrase_protected
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Profile{{id={}, name={:?}, email={:?}, plan={}, admin={}, passphrase_protected={}}}",
            self.id, self.name, self.email, self.plan_name, self.admin, self.passphrase_protected
        )
    }
}
