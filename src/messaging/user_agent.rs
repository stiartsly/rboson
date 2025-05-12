use std::path::Path;
use std::collections::LinkedList;
use std::collections::HashMap;

use crate::{
    Id,
    PeerInfo,
    error::Result,
    Error,
    core::crypto_identity::CryptoIdentity,
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
pub(crate) trait UserAgent {
    fn user(&self) -> Option<&UserProfile>;
    fn device(&self) -> Option<&DeviceProfile>;

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
pub struct DefaultUserAgent {
    user    : Option<UserProfile>,
    device  : Option<DeviceProfile>,
    peer    : Option<PeerInfo>,

    repository  : Option<MessagingRepository>,

    connection_listeners: LinkedList<Box<dyn ConnectionListener>>,
    profile_listeners   : LinkedList<Box<dyn ProfileListener>>,
    message_listeners   : LinkedList<Box<dyn MessageListener>>,
    channel_listeners   : LinkedList<Box<dyn ChannelListener>>,
    contact_listeners   : LinkedList<Box<dyn ContactListener>>,

    conversations: HashMap<Id, Conversation>,

    hardened: bool,
}

#[allow(unused)]
impl DefaultUserAgent {
    pub fn new(_path: Option<&Path>) -> Result<Self> {
        Ok(Self {
            user    : None,
            device  : None,
            peer    : None,

            repository: None,

            connection_listeners: LinkedList::new(),
            profile_listeners   : LinkedList::new(),
            message_listeners   : LinkedList::new(),
            channel_listeners   : LinkedList::new(),
            contact_listeners   : LinkedList::new(),

            conversations: HashMap::new(),

            hardened: false,
        })
    }

    pub(crate) fn set_user(&mut self, _user: &CryptoIdentity, name: &str) -> Result<()>{
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.user = Some(UserProfile::new(_user, name, false));
        self.update_user_info_config()
    }

    pub(crate) fn set_device(&mut self, device: &CryptoIdentity, name: &str, app: Option<&str>) -> Result<()> {
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.device = Some(DeviceProfile::new(Some(device), name, app));
        self.update_user_info_config()
    }

    fn update_user_info_config(&mut self) -> Result<()> {
        //unimplemented!()
        Ok(())
    }
}

impl UserAgent for DefaultUserAgent {
    fn user(&self) -> Option<&UserProfile> {
        self.user.as_ref()
    }

    fn device(&self) -> Option<&DeviceProfile> {
        self.device.as_ref()
    }

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