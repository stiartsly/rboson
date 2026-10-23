use std::{
    hash::Hash,
    sync::Arc,
    time::SystemTime
};
use boson_api::{
    Id,
    errors::{ArgumentError, Result}
};
use crate::{
    contact::{Contact, ContactOps, ContactType},
    invite_ticket::InviteTicket,
    message::Message,
};

/// A conversation between the local user and another party (person or channel).
///
/// The conversation ID equals the other party's boson `Id`.
pub trait ConversationOps: Send + Sync {
    /// The unique identifier of this conversation (same as the participant's `Id`).
    fn id(&self) -> &Id;

    /// The contact representing the other participant.
    fn contact(&self) -> &Contact;

    /// Whether this is a channel (group) conversation.
    fn is_channel(&self) -> bool;

    /// Short text preview of the most recent activity.
    fn preview(&self) -> Result<Option<&str>>;

    /// When this conversation was last updated.
    fn updated_at(&self) -> Option<SystemTime>;
}

    #[derive(Clone)]
pub struct Conversation {
    contact: Contact,
    last_message: Option<Arc<dyn Message>>,
    preview: Option<String>,
}

impl Conversation {
    pub(crate) const MAX_SNIPPET_LENGTH: usize = 128;

    pub(crate) fn new(contact: Contact) -> Self {
        Self {
            contact,
            last_message: None,
            preview: None
        }
    }

    pub(crate) fn update(&mut self, message: Arc<dyn Message>) -> Result<()> {
        if message.conversation_id()
                .map(|id| id != self.contact.id())
                .unwrap_or(true) {
            return Err(ArgumentError::new("Message does not match the conversation"));
        }

        self.preview = Self::preview_for(message.as_ref())?;
        self.last_message = Some(message);
        Ok(())
    }

    fn preview_for(message: &dyn Message) -> Result<Option<String>> {
        let Some(content) = message.payload_as_content() else {
            return Ok(None);
        };
        let content_type = content.content_type();

        let snippet = if content_type.starts_with("text/") {
            let Some(body) = content.as_text() else {
                return Ok(None);
            };
            let body = body.trim();
            if body.chars().count() < Self::MAX_SNIPPET_LENGTH {
                body.to_string()
            } else {
                format!("{}...", body.chars().take(Self::MAX_SNIPPET_LENGTH - 3).collect::<String>())
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

        Ok(Some(snippet))
    }

    pub(crate) fn is(&self, other: &Conversation) -> bool {
        self.id() == other.id()
    }
}

impl PartialEq for Conversation {
    fn eq(&self, other: &Self) -> bool {
        self.is(other)
    }
}

impl Hash for Conversation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        0x6030AC0A.hash(state);
        self.id().hash(state);
    }
}

impl ConversationOps for Conversation {
    fn id(&self) -> &Id {
        self.contact.id()
    }

    fn contact(&self) -> &Contact {
        &self.contact
    }

    fn is_channel(&self) -> bool {
        self.contact.contact_type() == ContactType::Channel
    }

    fn preview(&self) -> Result<Option<&str>> {
        Ok(self.preview.as_deref())
    }

    fn updated_at(&self) -> Option<SystemTime> {
        self.last_message.as_ref().and_then(|message| message.received_at().or(message.sent_at()))
    }
}
