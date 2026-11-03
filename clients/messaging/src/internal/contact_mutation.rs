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

use serde::{Deserialize, Serialize};

use boson_api::Id;
use super::OpaqueContact;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "d")]
pub(crate) enum MutationData {
    #[serde(rename = "a")]
    Add(OpaqueContact),
    #[serde(rename = "u")]
    Update(OpaqueContact),
    #[serde(rename = "r")]
    Remove(Vec<Id>),
    #[serde(rename = "c")]
    Clear,
}

/// A single revisioned contact mutation.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ContactMutation {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(flatten)]
    data: MutationData,
}

impl ContactMutation {
    fn new(revision: i32, data: MutationData) -> Self {
        Self { revision, data }
    }

    pub(crate) fn add(revision: i32, contact: OpaqueContact) -> Self {
        Self::new(revision, MutationData::Add(contact))
    }

    pub(crate) fn update(revision: i32, contact: OpaqueContact) -> Self {
        Self::new(revision, MutationData::Update(contact))
    }

    pub(crate) fn remove(revision: i32, ids: Vec<Id>) -> Self {
        Self::new(revision, MutationData::Remove(ids))
    }

    pub(crate) fn clear(revision: i32) -> Self {
        Self::new(revision, MutationData::Clear)
    }

    pub(crate) fn revision(&self) -> i32 {
        self.revision
    }

    pub(crate) fn data(&self) -> Option<&MutationData> {
        match &self.data {
            MutationData::Clear => None,
            data => Some(data),
        }
    }
}
