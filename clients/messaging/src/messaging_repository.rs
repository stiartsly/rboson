// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use serde::{de::DeserializeOwned, Serialize};

use boson_api::{
    Id,
    Result,
    Error,
};

use crate::{
    message::Message,
    channel::Channel,
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

    fn put_message(&self, _msg: Message) -> Result<()> {
        self.put_messages(&[_msg])
    }
    fn put_messages(&self, _msg: &[Message]) -> Result<()>;

    fn messages_between(&self, _conversation_id: &Id, _begin: u64, _end: u64) -> Result<Vec<Message>>;
    fn messages_since(&self, _conversation_id: &Id, _since: u64, _limit: usize, _offset: usize) -> Result<Vec<Message>>;

    fn remove_messages(&self, _rids: &[u32]) -> Result<()>;
    fn remove_messages_by_conversation(&self, _conversation_id: &Id) -> Result<()>;
    fn remove_amessage(&self, _rid: u32) -> Result<()> {
        self.remove_messages(&[_rid])
    }


    fn channel(&self, channel_id: &Id) -> Result<Option<Channel>>;
    fn channels(&self) -> Result<Vec<Channel>>;

}
