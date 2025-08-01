use std::fmt;
use std::time::SystemTime;

use crate::{
    Id,
    core::{Error, Result},
    messaging::{
        Contact,
        message::Message
    }
};

pub static MAX_SNIPPET_LENGTH: usize = 128;
pub static DEFAULT_AVATAR: Option<String> = None;

#[derive(Debug)]
pub struct Conversation {
    interlocutor    : Contact,
    last_message    : Option<Message>,
    snippet         : Option<String>,
}

#[allow(unused)]
impl Conversation {
    pub(crate) fn new(interlocutor: Contact, last_message: Message) -> Self {
        let mut conversation = Self {
            interlocutor,
            last_message: None,
            snippet: None,
        };
        conversation.update(last_message)
            .expect("Failed to update conversation with last message");
        conversation
    }

    pub(crate) fn from_contact(interlocutor: Contact) -> Self {
        Self {
            interlocutor,
            last_message: None,
            snippet: None,
        }
    }

    pub fn id(&self) -> &Id {
        self.interlocutor.id()
    }

    pub fn title(&self) -> String {
        self.interlocutor.display_name()
    }

    pub fn avatar(&self) -> Option<String> {
        self.interlocutor.avatar_url().or_else(|| DEFAULT_AVATAR.clone())
    }

    pub fn snippet(&self) -> String {
        if let Some(snippet) = self.snippet.as_ref() {
            return snippet.to_string();
        }
        let Some(msg) = self.last_message.as_ref() else {
            return "".into();
        };

        let ctype = msg.content_type();
        if ctype.starts_with("text/") {
            let body = msg.body_as_text().unwrap_or("".to_string());
            let trimmed = body.trim();
            let snippet = if trimmed.len() > MAX_SNIPPET_LENGTH {
                &trimmed[0..MAX_SNIPPET_LENGTH]
            } else {
                &trimmed[..]
            };
            snippet.to_string()
        } else if ctype.starts_with("image/") {
            "(Image)".into()
        } else if ctype.starts_with("audio/") {
            "(Audio)".into()
        } else if ctype.starts_with("video/") {
            "(Video)".into()
        } else {
            "(Attachment)".into()
        }
    }

    pub fn updated(&self) -> Option<SystemTime> {
        self.last_message.as_ref().map(|v| v.created())
    }

    pub fn iterlocutor(&self) -> &Contact {
        &self.interlocutor
    }

    pub(crate) fn update_interlocutor(&mut self, contact: Contact) -> Result<()> {
        if contact.id() != self.interlocutor.id() {
            Err(Error::Argument("Contact does not match the conversation".into()))?;
        }
        self.interlocutor = contact;
        Ok(())
    }

    pub(crate) fn update(&mut self, message: Message) -> Result<()> {
        if message.conversation_id() != self.interlocutor.id() {
            Err(Error::Argument("Message does not match the conversation".into()))?;
        }

        self.last_message = Some(message);
        self.snippet = None; // invalidate the previous snippet
        Ok(())
    }

    pub fn is(&self, conversation: &Conversation) -> bool {
        self.id() == conversation.id()
    }
}

impl PartialEq for Conversation {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl fmt::Display for Conversation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Conversation:{} [{}, {}]",
            self.title(),
            self.id(),
            self.snippet().as_str()
        )
    }
}
