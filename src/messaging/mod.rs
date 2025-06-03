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
pub(crate) mod invite_ticket;
pub mod message;
pub mod message_listener;
pub mod connection_listener;
pub mod config_adapter;
pub mod conversation;

pub mod client_device;

pub(crate) mod rpc;

pub mod messaging_repository;
pub mod service_ids;

pub(crate) mod client;
pub(crate) mod client_impl;
pub(crate) mod client_builder;

pub mod messaging_client {
    pub use crate::messaging::client::MessagingClient;
    pub use crate::messaging::client_impl::Client;
    pub use crate::messaging::client_builder::Builder;
}

pub(crate) mod user_agent;
pub(crate) mod user_agent_impl;
pub mod user_agent_ {
    pub use crate::messaging::user_agent_impl::DefaultUserAgent;
    pub use crate::messaging::user_agent::UserAgent;
}

pub use crate::{
    messaging::service_ids::ServiceIds,
    messaging::messaging_client::MessagingClient,
    messaging::messaging_client::Client,
    messaging::messaging_client::Builder as ClientBuilder,

    messaging::user_agent_::UserAgent,
    messaging::user_agent_::DefaultUserAgent,
    messaging::contact::Contact,
    messaging::contact_listener::ContactListener,
    messaging::conversation::Conversation,
    messaging::connection_listener::ConnectionListener,
    messaging::user_profile::UserProfile,
    messaging::device_profile::DeviceProfile,
    messaging::profile_listener::ProfileListener,
    messaging::message_listener::MessageListener,
    messaging::channel_listener::ChannelListener,
    messaging::invite_ticket::InviteTicket
};

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

mod bytes_as_base64 {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use base64::{engine::general_purpose, Engine as _};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
    {
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(D::Error::custom)
    }
}

#[allow(unused)]
mod id_as_base58 {
    use crate::Id;
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use bs58;

    pub fn serialize<S>(id: &Id, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = bs58::encode(id.as_bytes()).into_string();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Id, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Id::try_from(s.as_str()).map_err(D::Error::custom)?)
    }
}
