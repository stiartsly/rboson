/*
 * Copyright (c) 2023 -      bosonnetwork.io
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use crate::errors::{ArgumentError, Error, Result};
use super::{ContactMutation, OpaqueContact};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub(crate) enum SyncType {
    UpToDate = 0,
    Delta = 1,
    Snapshot = 2,
}

impl SyncType {
    pub(crate) fn value(self) -> i32 { self as i32 }
}

impl TryFrom<i32> for SyncType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::UpToDate),
            1 => Ok(Self::Delta),
            2 => Ok(Self::Snapshot),
            _ => Err(ArgumentError::new(format!("Invalid contact sync type: {value}"))),
        }
    }
}

/// A validated contact sync notification or response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "ContactSyncData")]
pub(crate) struct ContactSync {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(rename = "t")]
    kind: SyncType,
    #[serde(rename = "d", skip_serializing_if = "Vec::is_empty")]
    mutations: Vec<ContactMutation>,
    #[serde(rename = "s", skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<OpaqueContact>,
}

#[derive(Deserialize)]
struct ContactSyncData {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(rename = "t")]
    kind: SyncType,
    #[serde(rename = "d", default)]
    mutations: Option<Vec<ContactMutation>>,
    #[serde(rename = "s", default)]
    contacts: Option<Vec<OpaqueContact>>,
}

impl TryFrom<ContactSyncData> for ContactSync {
    type Error = Error;

    fn try_from(data: ContactSyncData) -> Result<Self> {
        Self::new(data.revision, data.kind, data.mutations.unwrap_or_default(), data.contacts.unwrap_or_default())
    }
}

impl ContactSync {
    fn new(revision: i32, kind: SyncType, mutations: Vec<ContactMutation>, contacts: Vec<OpaqueContact>) -> Result<Self> {
        if revision < 0 {
            return Err(ArgumentError::new("Contact sync revision is negative"));
        }
        if !mutations.is_empty() && !contacts.is_empty() {
            return Err(ArgumentError::new("Mutations and contacts can not be both present"));
        }
        match kind {
            SyncType::UpToDate if !mutations.is_empty() || !contacts.is_empty() =>
                return Err(ArgumentError::new("Up-to-date sync does not require mutations or contacts")),
            SyncType::Delta if mutations.is_empty() || !contacts.is_empty() =>
                return Err(ArgumentError::new("Delta sync requires mutations and no contacts")),
            SyncType::Snapshot if !mutations.is_empty() =>
                return Err(ArgumentError::new("Snapshot sync does not require mutations")),
            _ => {}
        }
        Ok(Self { revision, kind, mutations, contacts })
    }

    pub(crate) fn up_to_date(revision: i32) -> Result<Self> {
        Self::new(revision, SyncType::UpToDate, Vec::new(), Vec::new())
    }

    pub(crate) fn delta(revision: i32, mutations: Vec<ContactMutation>) -> Result<Self> {
        Self::new(revision, SyncType::Delta, mutations, Vec::new())
    }

    pub(crate) fn snapshot(revision: i32, contacts: Vec<OpaqueContact>) -> Result<Self> {
        Self::new(revision, SyncType::Snapshot, Vec::new(), contacts)
    }

    pub(crate) fn kind(&self) -> SyncType { self.kind }

    pub(crate) fn revision(&self) -> i32 { self.revision }

    pub(crate) fn mutations(&self) -> &[ContactMutation] { &self.mutations }

    pub(crate) fn contacts(&self) -> &[OpaqueContact] { &self.contacts }
}

#[cfg(test)]
#[path = "../unitests/test_contact_sync.rs"]
mod test_contact_sync;