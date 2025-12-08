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
    pub(crate) mod params;
    pub(crate) mod request;
    pub(crate) mod response;
    pub(crate) mod promise;
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
pub mod message_listener;

pub mod connection_listener;
pub mod config_adapter;
pub mod conversation;

pub mod client_device;

pub mod messaging_repository;
pub mod service_ids;

pub(crate) mod messaging_agent;
pub(crate) mod messaging_client;
pub(crate) mod messaging_client_builder;

pub mod client {
    pub use crate::messaging::messaging_agent::MessagingAgent;
    pub use crate::messaging::messaging_client::MessagingClient;
    pub use crate::messaging::messaging_client_builder::Builder;
}

pub(crate) mod user_agent_caps;
pub(crate) mod user_agent_impl;
pub mod user_agent_ {
    pub use crate::messaging::user_agent_impl::UserAgent;
    pub use crate::messaging::user_agent_caps::UserAgentCaps;
}

pub use crate::{
    messaging::client_device::ClientDevice,
    messaging::service_ids::ServiceIds,
    messaging::client::MessagingAgent,
    messaging::client::MessagingClient,
    messaging::client::Builder as ClientBuilder,

    messaging::user_agent_::UserAgentCaps,
    messaging::user_agent_::UserAgent,

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
