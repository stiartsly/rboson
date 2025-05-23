pub mod contact_listener;

pub mod persistence;
pub mod contact;
pub(crate) mod contact_sync_result;
pub(crate) mod contact_sequence;
pub(crate) mod contact_update;

pub(crate) mod channel;
pub(crate) mod channel_listener;

pub(crate) mod profile;
pub(crate) mod profile_listener;
pub mod user_profile;
pub mod device_profile;


pub(crate) mod api_client;

pub mod invite_ticket;
pub mod message;
pub mod messaging_client;
pub mod message_listener;
pub mod connection_listener;
pub mod config_adapter;
pub mod conversation;

pub mod client_device;

pub(crate) mod rpc;

pub mod user_agent;

pub mod messaging_repository;

pub mod client;

#[cfg(test)]
mod unitests;

pub(crate) fn is_none_or_empty_string(opt: &Option<String>) -> bool {
    match opt {
        Some(s) => s.is_empty(),
        None => true,
    }
}
pub(crate) fn is_false(b: &bool) -> bool {
    !b
}

pub(crate) fn is_zero<T: PartialEq + Default>(v: &T) -> bool {
    *v == T::default()
}

mod base64_as_string {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use base64::{engine::general_purpose, Engine as _};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
    {
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    #[allow(unused)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(D::Error::custom)
    }
}
