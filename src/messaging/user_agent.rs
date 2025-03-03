use std::collections::LinkedList;
use std::collections::HashMap;
use std::path::Path;

use crate::{
    Id,
    error::Result,
};

use super::{
    conversation::Conversation,
    message::Message,
};

#[allow(dead_code)]
trait IUserAgent {
    fn is_configured(&self) -> bool;
    fn list_conversations(&self) -> LinkedList<Conversation>;
    fn remove_conversation(&mut self, conversation_id: &Id);
    fn remove_conversations(&mut self, conversation_ids: Vec<&Id>);

    fn list_messages(&self, converstation_id: &Id) -> LinkedList<Message>;
    // TODO: list_messages_range(xxxx);

    fn remove_message(&mut self, message_id: &Id);
    fn remove_messages(&mut self, message_ids: Vec<&Id>);
    fn clear_messages(&mut self, conversation_id: &Id);
}

#[allow(dead_code)]
pub struct UserAgent {
    conversations: HashMap<Id, Conversation>,

}

impl UserAgent {
    pub fn new(_path: &Path) -> Result<Self> {
        unimplemented!()
    }
}

impl IUserAgent for UserAgent {
    fn is_configured(&self) -> bool {
        unimplemented!()
    }

    fn list_conversations(&self) -> LinkedList<Conversation> {
        unimplemented!()
    }

    fn remove_conversation(&mut self, _converstation_id: &Id) {
        unimplemented!()
    }

    fn remove_conversations(&mut self, _converstation_ids: Vec<&Id>) {
        unimplemented!()
    }

    fn list_messages(&self, _converstation_id: &Id) -> LinkedList<Message> {
        unimplemented!()
    }

    fn remove_message(&mut self, _message_id: &Id) {
        unimplemented!()
    }

    fn remove_messages(&mut self, _message_ids: Vec<&Id>) {
        unimplemented!()
    }

    fn clear_messages(&mut self, _converstation_id: &Id) {
        unimplemented!()
    }
}