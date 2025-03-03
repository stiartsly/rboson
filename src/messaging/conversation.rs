use std::fmt;
use std::time::SystemTime;

use crate::Id;
use super::{
    message::Message,
    contact::Contact
};

#[allow(dead_code)]
pub(crate) struct Conversation {
    id:             Id, // conversation id: the id of the interlocutor, could be a user or a group
    last_message:   Message,
    interlocutor:   Contact,
}

#[allow(dead_code)]
impl Conversation {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn title(&self) -> String {
        self.interlocutor.display_name()
    }

    pub fn avatar(&self) -> Option<&str> {
        self.interlocutor.avatar()
    }

    pub fn updated(&self) -> SystemTime {
        unimplemented!()
    }

    pub fn iterlocutor(&self) -> &Contact {
        unimplemented!()
    }

    pub fn snippet(&self) -> String {
        unimplemented!()
    }
}

impl PartialEq for Conversation {
    fn eq(&self, _other: &Self) -> bool {
		unimplemented!()
    }
}

impl fmt::Display for Conversation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Conversation:{} [{}, {}, {}]",
            self.title(),
            self.id,
            self.snippet(),
            "TODO"
        )?;
        Ok(())
    }
}
