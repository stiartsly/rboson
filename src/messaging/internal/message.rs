
use std::time::SystemTime;
use crate::Id;
use crate::messaging::message::{Message, Content};

pub(crate) struct PhotonMessage {
    pub(crate) id: Id,
    pub(crate) recipient: Id,
    pub(crate) from: Option<Id>,
    pub(crate) created_at: SystemTime,
    pub(crate) received_at: Option<SystemTime>,
    pub(crate) sent_at: Option<SystemTime>,
    pub(crate) payload: Vec<u8>,
    pub(crate) content: Content,
}

impl Message for PhotonMessage {
    fn id(&self) -> &Id { &self.id }
    fn rid(&self) -> i64 { 0 }
    fn conversation_id(&self) -> Option<&Id> {
        self.from.as_ref().or(Some(&self.recipient))
    }
    fn recipient(&self) -> &Id { &self.recipient }
    fn message_type(&self) -> crate::messaging::message::MessageType {
        crate::messaging::message::MessageType::ContentMessage
    }
    fn from(&self) -> Option<&Id> { self.from.as_ref() }
    fn created_at(&self) -> SystemTime { self.created_at }
    fn received_at(&self) -> Option<SystemTime> { self.received_at }
    fn sent_at(&self) -> Option<SystemTime> { self.sent_at }
    fn payload_as_bytes(&self) -> &[u8] { &self.payload }
    fn payload_as_content(&self) -> Option<&Content> { Some(&self.content) }
}
