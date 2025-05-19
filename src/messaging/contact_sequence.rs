
use std::fmt;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ContactSequence {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t")]
    timestamp: u64,
}

#[allow(unused)]
impl ContactSequence {
    pub(crate) fn new(id: String, timestamp: u64) -> Self {
        Self { id, timestamp }
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

pub(crate) fn gen_id() -> String {
    //let mut bin_id = [0u8; 16];
    //getrandom::getrandom(&mut bin_id).expect("Failed to generate random bytes");
    //hex::encode(bin_id)
    "TODO".to_string()
}
