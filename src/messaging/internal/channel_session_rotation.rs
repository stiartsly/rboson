use serde::{Deserialize, Serialize};

use crate::Id;

/// Session-key rotation request for a channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelSessionKeyRotation {
    #[serde(rename = "sid")]
    session_id: Id,
    #[serde(
        rename = "sk",
        serialize_with = "crate::utils::codec::bytes::serialize",
        deserialize_with = "crate::utils::codec::bytes::deserialize"
    )]
    session_key: Vec<u8>,
}

impl ChannelSessionKeyRotation {
    pub(crate) fn new(session_id: Id, session_key: Vec<u8>) -> Self {
        Self {
            session_id,
            session_key,
        }
    }
}
