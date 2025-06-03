use serde::{de::DeserializeOwned, Serialize};

use crate::{
    Id,
    error::Result,
    Error,
};

use crate::messaging::{
    message::Message
};

#[allow(unused)]
pub(crate) trait MessagingRepository {
    fn put_config(&self, _key: &str, _value: Vec<u8>)-> Result<()>;
    fn get_config(&self, _key: &str) -> Result<Vec<u8>>;

    fn put_config_mult<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        serde_json::to_vec(value).map_err(|e| {
            Error::Argument("Failed to serialize value for key {key}: {e}".into())
        }).and_then(|bytes| {
            self.put_config(key, bytes)
        })
    }

    fn get_config_mult<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let value = self.get_config(key).map_err(|e|
            Error::Argument("Key {key} not found".into())
        )?;
        let val = serde_json::from_slice(&value).map_err(|e| {
            Error::Argument("Failed to deserialize value for key {key}: {e}".into())
        })?;
        Ok(val)
    }

    fn put_messages(&self, _messages: &[Message]) -> Result<()>;
    fn put_message(&self, _message: Message) -> Result<()> {
        self.put_messages(&[_message])
    }

    fn messages_between(&self, _conversation_id: &Id, _begin: u64, _end: u64) -> Result<Vec<Message>>;
    fn messages_since(&self, _conversation_id: &Id, _since: u64, _limit: usize, _offset: usize) -> Result<Vec<Message>>;

    fn remove_messages(&self, _rids: &[u32]) -> Result<()>;
    fn remove_messages_by_conversation(&self, _conversation_id: &Id) -> Result<()>;
    fn remove_amessage(&self, _rid: u32) -> Result<()> {
        self.remove_messages(&[_rid])
    }
}
