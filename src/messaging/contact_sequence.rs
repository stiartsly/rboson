
use std::fmt;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use hex;

use crate::random_bytes;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ContactSequence {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t")]
    timestamp: u64,
}

#[allow(unused)]
impl ContactSequence {
    pub(crate) fn new() -> Self {
        let id = generate_id();
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap()
            .as_secs();

        Self {
            id: generate_id(),
            timestamp
        }
    }
    pub(crate) fn from_id(id: &str, timestamp: u64) -> Self {
        Self {
            id: id.into(),
            timestamp
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn timestamp(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp)
    }
}

impl fmt::Display for ContactSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Seq: {}/{}", self.id, self.timestamp)?;
        Ok(())
    }
}

fn generate_id() -> String {
    hex::encode(random_bytes(16).as_slice())
}
