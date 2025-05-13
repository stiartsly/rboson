
use std::path::Path;
use std::fs;
use log::error;

use crate::{
    Error,
    error::Result,
};

use crate::messaging::{
    messaging_repository::MessagingRepository,
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
    fn put_config(&self, _key: &str, _val: Vec<u8>) -> Result<()>  {
        unimplemented!()
    }

    fn get_config(&self, _key: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
}
