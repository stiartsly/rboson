use crate::{
    Id,
    PeerInfo,
    error::Result,
};

use crate::messaging::{
    Contact,
    Conversation,
    UserProfile,
    DeviceProfile,
};

use super::{
    message::Message,
    channel::Channel
};

#[allow(dead_code)]
pub trait UserAgent {
    fn user(&self) -> Option<&UserProfile>;
    fn device(&self) -> Option<&DeviceProfile>;
    fn peer_info(&self) -> Option<&PeerInfo>;

    fn is_configured(&self) -> bool;

    fn conversation(&self, _conversation_id: &Id) -> Option<Conversation>;
    fn conversations(&self) -> Vec<Conversation>;
    fn remove_conversation(&mut self, conversation_id: &Id);
    fn remove_conversations(&mut self, conversation_ids: Vec<&Id>);

    fn messages(&self, converstation_id: &Id) -> Vec<Message>;
    fn messages_between(&self, converstation_id: &Id, from: u64, end: u64) -> Vec<Message>;
    fn messages_since(&self, converstation_id: &Id, since: u64, limit: usize, offset: usize) -> Vec<Message>;

    fn remove_message(&mut self, message_id: u32);
    fn remove_messages(&mut self, message_ids: &[u32]);
    fn remove_messages_by_conversation(&mut self, conversation_id: &Id);

    fn channels(&self) -> Vec<Channel>;
    fn channel(&self, channel_id: &Id) -> Option<Channel>;

    fn contact_version(&self) -> Result<Option<String>>;
    fn put_contacts_update(&mut self, version_id: &str, contacts: &[Contact]) -> Result<()>;
}
