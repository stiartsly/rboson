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

use std::path::Path;
use std::fs;
use log::error;

use boson_api::{
    Id,
    Error,
    Result,
};

use crate::{
    messaging_repository::MessagingRepository,
    message::Message,
    channel::Channel,
};

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Database;

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

    fn channel(&self, _channel_id: &Id) -> Result<Option<Channel>> {
        // unimplemented!()
        Ok(None)
    }

    fn channels(&self) -> Result<Vec<Channel>> {
        unimplemented!()
    }
}
