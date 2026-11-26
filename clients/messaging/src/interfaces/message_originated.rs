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

use sha2::{Digest, Sha256};
use boson_api::Id;

pub trait MessageOriginated {
    fn id(&self) -> &Id;

    fn timestamp(&self) -> i64;

    fn is_originated(&self, device_id:&Id) -> bool {
        let timestamp = self.timestamp();
        match Self::try_generate_id(device_id, timestamp) {
            Some(id) => id == *self.id(),
            None => false,
        }
    }

    fn try_generate_id(device_id: &Id, timestamp: i64) -> Option<Id> {
        let mut sha = Sha256::new();
        sha.update(device_id.as_bytes());
        sha.update(&timestamp.to_le_bytes());
        let bytes = sha.finalize().to_vec();

        Id::try_from_bytes(&bytes).ok()
    }

    fn generate_id(device_id: &Id, timestamp: i64) -> Id {
        Self::try_generate_id(device_id, timestamp)
            .expect("Failed to generate ID")
    }
}

pub(crate) fn generate_id(device_id: &Id, timestamp: i64) -> Id {
    let mut sha = Sha256::new();
    sha.update(device_id.as_bytes());
    sha.update(&timestamp.to_le_bytes());
    let bytes = sha.finalize().to_vec();

    Id::try_from_bytes(&bytes).unwrap()
}
