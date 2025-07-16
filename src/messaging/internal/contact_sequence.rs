
use std::fmt;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use hex;

use crate::{
    as_ms,
    random_bytes
};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ContactSequence {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t")]
    timestamp: u64,
}

#[allow(unused)]
impl ContactSequence {
    pub(crate) fn default() -> Self {
        let generate_id = || hex::encode(random_bytes(16).as_slice());
        Self {
            id: generate_id(),
            timestamp: as_ms!(SystemTime::now()) as u64
        }
    }
    pub(crate) fn new(id: &str, timestamp: u64) -> Self {
        Self {
            id: id.to_string(),
            timestamp
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn timestamp(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.timestamp)
    }
}

impl fmt::Display for ContactSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Seq: {}/{}", self.id, self.timestamp)
    }
}
