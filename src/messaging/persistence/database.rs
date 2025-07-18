
use std::path::Path;
use std::fs;
use log::error;

use crate::{
    Id,
    Error,
    error::Result,
};

use crate::messaging::{
    messaging_repository::MessagingRepository,
    message::Message,
};

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Database {
}

#[allow(unused)]
impl Database {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(e) => {
                fs::create_dir_all(path).map_err(|e| {
                    Error::Argument(format!("Failed to create directory {}: {e}", path.display()))
                })?;
                fs::metadata(path).map_err(|e| {
                    Error::Argument(format!("Failed to get metadata for path {}: {e}", path.display()))
                })?
            }
        };

        if !metadata.is_dir() {
            Err(Error::Argument(format!("Path {} is not a directory", path.display())))?;
        }

        let _path = fs::canonicalize(path).map_err(|e| {
            error!("{e}, path: {}", path.display());
            Error::Argument(format!("Invalid persistent path {} with error: {e}", path.display()))
        })?;

        Ok(Database {})
    }
}

impl MessagingRepository for Database {
    fn put_config(&self, _key: &str, _val: Vec<u8>) -> Result<()>  {
        unimplemented!()
    }

    fn get_config(&self, _key: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn put_messages(&self, _messages: &[Message]) -> Result<()> {
        unimplemented!()
    }

    fn messages_between(&self, _conversation_id: &Id, _begin: u64, _end: u64) -> Result<Vec<Message>> {
        unimplemented!()
    }

    fn messages_since(&self, _conversation_id: &Id, _since: u64, _limit: usize, _offset: usize) -> Result<Vec<Message>> {
        unimplemented!()
    }

    fn remove_messages(&self, _rids: &[u32]) -> Result<()> {
        unimplemented!()
    }

    fn remove_messages_by_conversation(&self, _conversation_id: &Id) -> Result<()> {
        unimplemented!()
    }
}
