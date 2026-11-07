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

use std::time::{SystemTime, Duration, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use serde_cbor::Value;

use boson_api::{
    utils,
    Id,
};
use crate::{
    MessageOps,
    MessageContentOps,
    MessageType,
};
use super::photon_content::PhotonContent;

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PhotonMessage {
    #[serde(skip)]
    pub(crate) rid: i64,

    #[serde(rename = "v")]
    pub(crate) version: u8,

    #[serde(rename = "id", with = "utils::id")]
    pub(crate) id: Id,

    #[serde(rename = "r", with = "utils::id")]
    pub(crate) recipient: Id,

    #[serde(rename = "y")]
    pub(crate) message_type: u8,

    #[serde(
        rename = "f",
        default,
        with = "utils::id_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) from: Option<Id>,

    #[serde(
        rename = "c",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    pub(crate) created_at: u64,

    #[serde(rename = "p")]
    pub(crate) payload: Value,

    #[serde(skip)]
    pub(crate) received_at: Option<SystemTime>,

    #[serde(skip)]
    pub(crate) sent_at: Option<SystemTime>,

    #[serde(skip)]
    pub(crate) payload_bytes: Vec<u8>,

    #[serde(skip)]
    pub(crate) content: Option<PhotonContent>,

    #[serde(skip)]
    pub(crate) conversation_id: Option<Id>,
}

impl PhotonMessage {
    pub(crate) fn new(
        version: u8,
        id: Id,
        recipient: Id,
        message_type: u8,
        from: Option<Id>,
        created_at: u64,
        payload: Value,
    ) -> Self {
        Self {
            rid: 0,
            version,
            id,
            recipient,
            message_type,
            from,
            created_at,
            payload,
            received_at: None,
            sent_at: None,
            payload_bytes: Vec::new(),
            content: None,
            conversation_id: None,
        }
    }
}

impl MessageOps for PhotonMessage {
    fn id(&self) -> &Id {
        &self.id
    }

    fn rid(&self) -> i64 {
        self.rid
    }

    fn conversation_id(&self) -> Option<&Id> {
        self.conversation_id.as_ref().or_else(|| {
            if self.received_at.is_some() {
                self.from.as_ref()
            } else {
                Some(&self.recipient)
            }
        })
    }

    fn recipient(&self) -> &Id {
        &self.recipient
    }

    fn message_type(&self) -> MessageType {
        // Only content messages are surfaced through the public listener API.
        MessageType::Content
    }

    fn from(&self) -> Option<&Id> {
        self.from.as_ref()
    }

    fn created_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.created_at)
    }

    fn received_at(&self) -> Option<SystemTime> {
        self.received_at
    }

    fn sent_at(&self) -> Option<SystemTime> {
        self.sent_at
    }

    fn payload_as_bytes(&self) -> &[u8] {
        &self.payload_bytes
    }

    fn payload_as_content(&self) -> Option<Box<dyn MessageContentOps>> {
        self.content.clone().map(|content| Box::new(content) as Box<dyn MessageContentOps>)
    }

    fn clone_box(&self) -> Box<dyn MessageOps> {
        Box::new(self.clone())
    }
}


#[cfg(test)]
#[path = "../unitests/test_message.rs"]
mod test_message;
