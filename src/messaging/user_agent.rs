use crate::{
    Id,
    PeerInfo,
    core::Result,
};

use crate::messaging::{
    Contact,
    Conversation,
    UserProfile,
    DeviceProfile,
    ConnectionListener,
    ProfileListener,
    MessageListener,
    ChannelListener,
    ContactListener,
    message::Message,
    channel::Channel
};

pub trait UserAgent: Send + ConnectionListener + ChannelListener {
    fn user(&self) -> Option<&UserProfile>;
    fn device(&self) -> Option<&DeviceProfile>;
    fn peer(&self) -> &PeerInfo;

    fn is_configured(&self) -> bool;
    fn harden(&mut self);

    fn add_connection_listener(&mut self, listener: Box<dyn ConnectionListener>);
    fn add_profile_listener(&mut self, listener: Box<dyn ProfileListener>);
    fn add_message_listener(&mut self, listener: Box<dyn MessageListener>);
    fn add_channel_listener(&mut self, listener: Box<dyn ChannelListener>);
    fn add_contact_listener(&mut self, listener: Box<dyn ContactListener>);

    fn conversation(&self, _conversation_id: &Id) -> Option<&Conversation>;
    fn conversations(&self) -> Vec<&Conversation>;
    fn remove_conversation(&mut self, conversation_id: &Id);
    fn remove_conversations(&mut self, conversation_ids: Vec<&Id>);

    fn messages(&self, converstation_id: &Id) -> Vec<Message>;
    fn messages_between(&self, converstation_id: &Id, from: u64, end: u64) -> Vec<Message>;
    fn messages_since(&self, converstation_id: &Id, since: u64, limit: usize, offset: usize) -> Vec<Message>;

    fn remove_message(&mut self, message_id: u32);
    fn remove_messages(&mut self, message_ids: &[u32]);
    fn remove_messages_by_conversation(&mut self, conversation_id: &Id);

    fn channels(&self) -> Result<Vec<&Channel>>;
    fn channel(&self, channel_id: &Id) -> Result<Option<Channel>>;

    fn contact_version(&self) -> Result<Option<String>>;
    fn put_contacts_update(&mut self, version_id: &str, contacts: &[Contact]) -> Result<()>;

    fn contact(&self, id: &Id) -> Result<Option<Contact>>;
    fn contacts(&self) -> Result<Vec<Contact>>;

    fn remove_contact(&mut self, id: &Id) -> Result<()>;
    fn remove_contacts(&mut self, ids: Vec<&Id>) -> Result<()>;
}
