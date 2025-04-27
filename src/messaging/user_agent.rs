use std::collections::LinkedList;
use std::collections::HashMap;
use std::path::Path;

use crate::{
    Id,
    error::Result,
    PeerInfo,
};

use super::{
    conversation::Conversation,
    message::Message,
    user_profile::UserProfile,
    device_profile::DeviceProfile,
    connection_listener::ConnectionListener,
    profile_listener::ProfileListener,
    message_listener::MessageListener,
    channel_listener::ChannelListener,
    contact_listener::ContactListener
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

struct MessagingRepository {}

#[allow(dead_code)]
pub struct UserAgent {
    user    : UserProfile,
    device  : DeviceProfile,
    peer    : PeerInfo,

    repository  : MessagingRepository,

    connection_listeners: LinkedList<Box<dyn ConnectionListener>>,
    profile_listeners: LinkedList<Box<dyn ProfileListener>>,
    message_listeners: LinkedList<Box<dyn MessageListener>>,
    channel_listeners: LinkedList<Box<dyn ChannelListener>>,
    contact_listeners: LinkedList<Box<dyn ContactListener>>,

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