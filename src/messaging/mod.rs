pub(crate) mod internal;
pub(crate) mod rpc;

pub(crate) mod mqtt;
pub(crate) mod rpc_server;
pub(crate) mod verticle;

pub mod client;
pub mod options;
pub mod channel;
pub mod channel_member;
pub mod contact;
pub mod contact_editor;
pub mod errors;
pub mod conversation;
pub mod friend_request;
pub mod invite_ticket;
pub mod message;
pub mod content_type;
pub mod content_disposition;

pub mod session_info;
pub mod messaging_client;

pub mod channel_listener;
pub mod connection_listener;
pub mod contact_listener;
pub mod friend_request_listener;
pub mod message_listener;
pub mod session_listener;

pub use {
    messaging_client::MessagingClient,
    options::{Options, OptionsBuilder},
    client::{Client},
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    connection_listener::ConnectionListener,
    contact_listener::ContactListener,
    message_listener::MessageListener,
    session_listener::SessionListener,
    channel_listener::ChannelListener,
    channel::{Channel, ChannelEditor, Permission, Role},
    channel_member::ChannelMember,
    contact::{Contact, ContactOps, ContactType},
    contact_editor::ContactEditor,
    conversation::ConversationOps,
    invite_ticket::InviteTicket,
    session_info::SessionInfo,
};

pub use message::{
    Content, ContentDisposition, Type, Message, MessageBuilder,
    MessageType, CONTENT_DISPOSITION_HEADER,
};

pub(crate) use internal::ClientConnectionListener;

#[cfg(test)]
mod unitests {
    mod test_message;
    mod test_contact;
   // mod test_mqtt;
   // mod test_session;
   // mod test_verticle;
}
