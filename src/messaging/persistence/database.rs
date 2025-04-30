
use std::path::Path;
use std::fs;
use log::error;

use crate::{
    Error,
    error::Result,
};

use crate::messaging::{
    messaging_repository::MessagingRepository,
    message::Message,
};

#[allow(unused)]
pub(crate) struct Database {
}

#[allow(unused)]
impl Database {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let path = fs::canonicalize(path).map_err(|e| {
            error!("{e}");
            Error::Argument(format!("Invalid persistent path {} with error: {e}", path.display()))
        })?;

        if path.exists() {
            // TODO:
        }

        let _ = fs::metadata(path).map_err(|e| {
            error!("{e}");
            Error::Argument(format!("Internal error: {e}"))
        });

        Ok(Database {})
    }
}

impl MessagingRepository for Database {
    fn put_config(&self, _: &str, _: &[u8]) -> Result<()>  {
        unimplemented!()
    }

    fn get_config(&self, _: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn put_msg(&self, _:&Message) -> Result<()> {
        unimplemented!()
    }

    fn put_messages(&self, _: &[Message]) -> Result<()> {
        unimplemented!()
    }

    fn remove_msg(&self, _: u32) {
        unimplemented!()
    }
}
