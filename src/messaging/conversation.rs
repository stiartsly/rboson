use std::fmt;
use std::time::SystemTime;

use crate::{
    Id,
    error::Result,
    Error,
    messaging::Contact
};

use super::{
    message::Message,
};

pub static MAX_SNIPPET_LENGTH: usize = 128;
pub static DEFAULT_AVATAR: Option<String> = None;

pub struct Conversation {
    interlocutor: Contact,
    last_message: Option<Message>,
    snippet     : Option<String>,
}

#[allow(unused)]
impl Conversation {
    pub fn id(&self) -> &Id {
        self.interlocutor.id()
    }

    pub fn title(&self) -> String {
        self.interlocutor.display_name()
    }

    pub fn avatar(&self) -> Option<String> {
        self.interlocutor.avatar_url()
    }

    pub fn snippet(&self) -> String {
        if let Some(snippet) = self.snippet.as_ref() {
            return snippet.to_string();
        }

        let Some(msg) = self.last_message.as_ref() else {
            return "".to_string();
        };

        let ctype = msg.content_type();
        if ctype.starts_with("text/") {
            let body = msg.body_as_text();
            let trimmed = body.trim();
            let snippet = if trimmed.len() > MAX_SNIPPET_LENGTH {
                &trimmed[0..MAX_SNIPPET_LENGTH]
            } else {
                &trimmed[..]
            };
            snippet.to_string()
        } else if ctype.starts_with("image/") {
            "(Image)".to_string()
        } else if ctype.starts_with("audio/") {
            "(Audio)".to_string()
        } else if ctype.starts_with("video/") {
            "(Video)".to_string()
        } else {
            "(Attachment)".to_string()
        }
    }

    pub(crate) fn udpate_snippet(&mut self) {
        if self.snippet.is_none() {
            self.snippet = Some(self.snippet())
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
            return Err(Error::Argument("Contact does not match the conversation".into()))
        }
        self.interlocutor = contact;
        Ok(())
    }

    pub(crate) fn update(&mut self, message: Message) -> Result<()> {
        if message.conversation_id() != self.id() {
            return Err(Error::Argument("Message does not match the conversation".into()))
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
        write!(f, "Conversation:{} [{}, {}, {}]",
            self.title(),
            self.id().to_base58(),
            self.snippet().as_str(),
            "TODO"
        )?;
        Ok(())
    }
}
