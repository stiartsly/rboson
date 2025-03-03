
use std::path::Path;
use std::fs;
use log::error;

use crate::{
    Error,
    error::Result,
};

#[allow(dead_code)]
pub(crate) struct Database {

}

#[allow(dead_code)]
impl Database {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let path = fs::canonicalize(path).map_err(|e| {
            error!("{e}");
            Error::Argument(format!("Invalid persistent path {} with error: {e}", path.display()))
        })?;

        if path.exists() {
            // TODO:
        }

        let exist = fs::metadata(path).map_err(|e| {
            error!("{e}");
            Error::Argument(format!("Internal error: {e}"))
        });

        unimplemented!()
    }
}
