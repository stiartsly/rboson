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

use std::time::SystemTime;
use serde::{Serialize, Deserialize};
use serde_cbor::Value;

use boson_api::{utils, Id};
use crate::{
    MessageOps,
    MessageContentOps,
    MessageOriginated,
    MessageType,
};
use super::photon_content::PhotonContent;

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PhotonMessage {
    #[serde(rename = "v")]
    version: i32,

    #[serde(rename = "id", with = "utils::id")]
    id: Id,

    #[serde(rename = "r", with = "utils::id")]
    recipient: Id,

    #[serde(rename = "y")]
    message_type: i32,

    #[serde(
        rename = "f",
        default,
        with = "utils::id_opt",
    )]
    from: Option<Id>,

    #[serde(
        rename = "c",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    created_at: u64,

    #[serde(rename = "p")]
    pub(crate) payload: Value,


    #[serde(skip)]
    rid: i64,
    #[serde(skip)]
    received_at: Option<u64>,
    #[serde(skip)]
    sent_at: Option<u64>,
    #[serde(skip)]
    pub(crate)  payload_bytes: Vec<u8>,
    #[serde(skip)]
    pub(crate) content: Option<PhotonContent>,
    #[serde(skip)]
    conversation_id: Option<Id>,
}

impl PhotonMessage {
    pub(crate) fn new(
        version: i32,
        id: Id,
        recipient: Id,
        message_type: i32,
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

    pub(crate) fn set_converstation_id(&mut self, conv_id: Id) {
        self.conversation_id = Some(conv_id);
    }

    pub(crate) fn set_from(&mut self, from: Id) {
        self.from = Some(from);
    }

    pub(crate) fn set_sent(&mut self, sent_at: u64) {
        self.sent_at = Some(sent_at);
    }

    pub(crate) fn set_received_at(&mut self, received_at: u64) {
        self.received_at = Some(received_at);
    }

    pub(crate) fn set_received(&mut self) {
        self.received_at = Some(boson_api::as_ms!(SystemTime::now()) as u64);
    }

    pub(crate) fn set_rid(&mut self, rid: i64) {
        self.rid = rid;
    }

    pub(crate) fn from_or_throw(&self) -> boson_api::Result<&Id> {
        self.from.as_ref().ok_or_else(|| {
            boson_api::errors::StateError::new("Message sender is not set").into()
        })
    }

    pub(crate) fn version(&self) -> i32 {
        self.version
    }

    pub(crate) fn created_at_u64(&self) -> u64 {
        self.created_at
    }

    pub(crate) fn sent_at_u64(&self) -> Option<u64> {
        self.sent_at
    }

    pub(crate) fn received_at_u64(&self) -> Option<u64> {
        self.received_at
    }
}

impl MessageOriginated for PhotonMessage {
    fn id(&self) -> &Id {
        &self.id
    }

    fn timestamp(&self) -> u64 {
        self.created_at
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
        self.conversation_id.as_ref()
    }

    fn recipient(&self) -> &Id {
        &self.recipient
    }

    fn message_type(&self) -> MessageType {
        MessageType::try_from(self.message_type).unwrap()
    }

    fn from(&self) -> Option<&Id> {
        self.from.as_ref()
    }

    fn created_at(&self) -> SystemTime {
        boson_api::from_ms!(self.created_at)
    }

    fn received_at(&self) -> Option<SystemTime> {
        self.received_at.map(|ts| boson_api::from_ms!(ts))
    }

    fn sent_at(&self) -> Option<SystemTime> {
        self.sent_at.map(|ts| boson_api::from_ms!(ts))
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
