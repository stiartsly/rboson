pub(crate) mod internal;
pub(crate) mod rpc;

pub(crate) mod mqtt;
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

};

pub use channel::{Channel, ChannelEditor, Permission, Role};
pub use channel_member::ChannelMember;
pub use contact::{Contact, ContactOps, ContactType};
pub use contact_editor::ContactEditor;
pub use conversation::ConversationOps;
pub use invite_ticket::InviteTicket;
pub use message::{
    content_type, Content, ContentDisposition, ContentDispositionType, Message, MessageBuilder,
    MessageType, CONTENT_DISPOSITION_HEADER,
};
pub use session_info::SessionInfo;

pub(crate) use {
    internal::{
        client_connection_listener::ClientConnectionListener
    },
};

#[cfg(test)]
mod unitests {
    mod test_message;
    mod test_contact;
   // mod test_mqtt;
   // mod test_session;
   // mod test_verticle;
}
