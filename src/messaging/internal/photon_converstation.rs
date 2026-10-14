use std::{cmp::Ordering, sync::Arc, time::SystemTime};
use crate::{Id, errors::{ArgumentError, Result}};
use crate::messaging::{
    contact::{Contact, ContactType},
    conversation::Conversation,
    invite_ticket::InviteTicket,
    message::Message,
};
use super::PhotonContact;

#[derive(Clone)]
pub(crate) struct PhotonConversation {
    contact: PhotonContact,
    last_message: Option<Arc<dyn Message>>,
    preview: String,
}

impl PhotonConversation {
    pub(crate) const MAX_SNIPPET_LENGTH: usize = 128;

    pub(crate) fn new(contact: PhotonContact) -> Self {
        Self { contact, last_message: None, preview: String::new() }
    }

    pub(crate) fn update(&mut self, message: Arc<dyn Message>) -> Result<()> {
        if message.conversation_id() != Some(self.contact.id()) {
            return Err(ArgumentError::new("Message does not match the conversation"));
        }
        let content = message.payload_as_content()
            .ok_or_else(|| ArgumentError::new("Conversation message content is missing"))?;
        let content_type = content.content_type();
        let preview = if content_type.starts_with("text/") {
            let text = content.as_text()
                .ok_or_else(|| ArgumentError::new("Conversation text is not UTF-8"))?.trim();
            if text.chars().count() < Self::MAX_SNIPPET_LENGTH {
                text.to_string()
            } else {
                format!("{}...", text.chars().take(Self::MAX_SNIPPET_LENGTH - 3).collect::<String>())
            }
        } else if content_type.starts_with("image/") {
            "(Image)".into()
        } else if content_type.starts_with("audio/") {
            "(Audio)".into()
        } else if content_type.starts_with("video/") {
            "(Video)".into()
        } else if content_type.starts_with(InviteTicket::CONTENT_TYPE) {
            "(Invitation)".into()
        } else if content.content_disposition()?.is_some() {
            "(Attachment)".into()
        } else {
            "(Unknown)".into()
        };
        self.preview = preview;
        self.last_message = Some(message);
        Ok(())
    }

    pub(crate) fn compare(&self, other: &dyn Conversation) -> Ordering {
        self.updated_at().cmp(&other.updated_at()).then_with(|| self.id().cmp(other.id()))
    }
}

impl Conversation for PhotonConversation {
    fn id(&self) -> &Id { self.contact.id() }
    fn title(&self) -> &str { self.contact.remark().or(self.contact.name()).unwrap_or("") }
    fn avatar(&self) -> Option<&str> { None }
    fn contact(&self) -> &dyn Contact { &self.contact }
    fn is_channel(&self) -> bool { self.contact.contact_type() == ContactType::Channel }
    fn preview(&self) -> &str { &self.preview }
    fn updated_at(&self) -> Option<SystemTime> {
        self.last_message.as_ref().and_then(|message| message.received_at().or(message.sent_at()))
    }
    fn message_count(&self) -> usize { usize::from(self.last_message.is_some()) }
    fn unread_count(&self) -> usize {
        usize::from(self.last_message.as_ref().is_some_and(|message| message.received_at().is_some()))
    }
    fn is_muted(&self) -> bool { self.contact.is_muted() }
    fn is_pinned(&self) -> bool { false }
}
