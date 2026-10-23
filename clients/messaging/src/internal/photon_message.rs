use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use crate::messaging::message::{Content, Message, MessageType};
use crate::{utils, Id};

/// Message envelope as it appears on the wire.
///
/// The local-only fields (`received_at`, `sent_at`, `payload_bytes`, `content`)
/// are filled by the transport layer after decoding and are never serialized.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PhotonMessage {
    #[serde(rename = "v")]
    pub(crate) version: u8,

    #[serde(rename = "id")]
    pub(crate) id: Id,

    #[serde(rename = "r")]
    pub(crate) recipient: Id,

    #[serde(rename = "y")]
    pub(crate) message_type: u8,

    #[serde(
        rename = "f",
        default,
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
    pub(crate) content: Option<Content>,
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
        }
    }
}

impl Message for PhotonMessage {
    fn id(&self) -> &Id {
        &self.id
    }

    fn rid(&self) -> i64 {
        0
    }

    fn conversation_id(&self) -> Option<&Id> {
        if self.received_at.is_some() {
            self.from.as_ref()
        } else {
            Some(&self.recipient)
        }
    }

    fn recipient(&self) -> &Id {
        &self.recipient
    }

    fn message_type(&self) -> MessageType {
        // Only content messages are surfaced through the public listener API.
        MessageType::ContentMessage
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

    fn payload_as_content(&self) -> Option<&Content> {
        self.content.as_ref()
    }
}
