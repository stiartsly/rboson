use serde::Serialize;
use boson_api::{utils, Id};

#[derive(Debug, Serialize)]
pub(crate) struct JoinChannel {
    #[serde(rename = "c", with = "utils::id")]
    channel_id: Id,
    #[serde(rename = "sid", with = "utils::id")]
    session_id: Id,
    #[serde(rename = "i", with = "utils::id")]
    inviter: Id,
    #[serde(rename = "e")]
    expiration_ms: u64,
    #[serde(rename = "sig", with = "utils::bytes")]
    signature: Vec<u8>,
    #[serde(rename = "sk", with = "utils::bytes")]
    session_key: Vec<u8>,
    #[serde(rename = "ie", with = "utils::id_opt")]
    invitee: Option<Id>,
}

impl JoinChannel {
    pub(crate) fn new(
        channel_id: Id,
        session_id: Id,
        inviter: Id,
        expiration_ms: u64,
        signature: Vec<u8>,
        session_key: Vec<u8>,
        invitee: Option<Id>,
    ) -> Self {
        Self {
            channel_id,
            session_id,
            inviter,
            expiration_ms,
            signature,
            session_key,
            invitee,
        }
    }
}
