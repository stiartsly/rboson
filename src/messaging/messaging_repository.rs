
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::Result,
    Error,
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




    //fn put_msg(&self, _:&Message) -> Result<()>;
    //fn put_messages(&self, _: &[Message]) -> Result<()>;

    //fn remove_msg(&self, _: u32);
}
