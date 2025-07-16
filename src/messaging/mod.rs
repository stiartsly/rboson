pub(crate) mod persistence {
    pub(crate) mod database;
}

pub(crate) mod internal {
    pub(crate) mod contact_sequence;
    pub(crate) mod contacts_update;
    pub(crate) mod contact_sync_result;

    pub(crate) use self::{
        contacts_update::ContactsUpdate,
        // contact_sequence::ContactSequence,
        // contact_sync_result::ContactSyncResult
    };
}

pub(crate) mod rpc {
    pub(crate) mod method;
    pub(crate) mod error;
    pub(crate) mod parameters;
    pub(crate) mod request;
    pub(crate) mod response;
}

pub(crate) mod contact;
pub(crate) mod contact_listener;

pub(crate) mod channel;
pub(crate) mod channel_listener;

pub(crate) mod profile;
pub(crate) mod profile_listener;
pub mod user_profile;
pub mod device_profile;


pub(crate) mod api_client;
pub(crate) mod invite_ticket;

pub mod message;
pub mod message_builder;
pub mod message_listener;

pub mod connection_listener;
pub mod config_adapter;
pub mod conversation;

pub mod client_device;

pub mod messaging_repository;
pub mod service_ids;

pub(crate) mod messaging_client;
pub(crate) mod client_impl;
pub(crate) mod client_builder;

pub mod client {
    pub use crate::messaging::messaging_client::MessagingClient;
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
    messaging::client_device::ClientDevice,
    messaging::service_ids::ServiceIds,
    messaging::client::MessagingClient,
    messaging::client::Client,
    messaging::client::Builder as ClientBuilder,

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
    messaging::invite_ticket::InviteTicket,

    messaging::channel::{Role, Member, Permission, Channel},
    messaging::message::{
        Message,
        ContentType,
        ContentDisposition
    },
};

#[cfg(test)]
mod unitests {
    mod test_api_client;
    mod test_user_profile;
    mod test_client_device;
    mod test_user_agent;
    mod test_invite_ticket;
    mod test_conversation;
    // mod test_contact;
    mod test_channel;
    mod test_client;
}

fn is_default<T: IsDefault>(v: &T) -> bool {
    v.is_default()
}

trait IsDefault {
    fn is_default(&self) -> bool;
}

impl IsDefault for u64 {
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl IsDefault for bool {
    fn is_default(&self) -> bool {
        !*self
    }
}

impl IsDefault for String {
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

impl<T> IsDefault for Vec<T> {
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

// bytes serded as base64 URL safe without padding
mod serde_bytes_with_base64 {
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

// serde Id as base58 string
#[allow(unused)]
mod serde_id_with_base58 {
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
