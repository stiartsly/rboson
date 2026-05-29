use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;

use crate::Id;
use crate::messaging::{
    errors::Result,
    channel::Permission,
    contact::Contact,
    channel::Channel,
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact_listener::ContactListener,
    conversation::Conversation,
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::Message,
    message::MessageBuilder,
    message_listener::MessageListener,
    session_info::SessionInfo,
    session_listener::SessionListener,
};

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// A boxed future returned by async methods on [`MessagingClient`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The public API surface of the boson messaging client.
///
/// All network / async methods return a [`BoxFuture`] so the trait stays
/// object-safe and the implementation can use any async runtime internally.
pub trait MessagingClient: Send + Sync {
    // -----------------------------------------------------------------
    // Identity
    // -----------------------------------------------------------------

    /// The boson `Id` of the authenticated user.
    fn user_id(&self) -> &Id;

    /// The boson `Id` of the current device.
    fn device_id(&self) -> &Id;

    /// The boson `Id` of the messaging service peer.
    fn service_peer_id(&self) -> &Id;

    /// The MQTT endpoint of the connected service, if known.
    fn service_endpoint(&self) -> Option<&str>;

    /// The local data directory used for persistence.
    fn data_dir(&self) -> &std::path::Path;

    // -----------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------

    /// Start the client and connect to the service.
    fn start(&self) -> BoxFuture<'_, Result<()>>;

    /// Gracefully stop the client.
    fn stop(&self) -> BoxFuture<'_, Result<()>>;

    /// Whether the client background worker is running.
    fn is_running(&self) -> bool;

    /// Whether the MQTT connection is currently established.
    fn is_connected(&self) -> bool;

    /// Whether the client is connected *and* fully initialised.
    fn is_ready(&self) -> bool;

    // -----------------------------------------------------------------
    // Listeners
    // -----------------------------------------------------------------

    fn add_connection_listener(&self, listener: Arc<dyn ConnectionListener>);
    fn remove_connection_listener(&self, listener: &Arc<dyn ConnectionListener>);

    fn add_message_listener(&self, listener: Arc<dyn MessageListener>);
    fn remove_message_listener(&self, listener: &Arc<dyn MessageListener>);

    fn add_channel_listener(&self, listener: Arc<dyn ChannelListener>);
    fn remove_channel_listener(&self, listener: &Arc<dyn ChannelListener>);

    fn add_contact_listener(&self, listener: Arc<dyn ContactListener>);
    fn remove_contact_listener(&self, listener: &Arc<dyn ContactListener>);

    fn add_session_listener(&self, listener: Arc<dyn SessionListener>);
    fn remove_session_listener(&self, listener: &Arc<dyn SessionListener>);

    fn add_friend_request_listener(&self, listener: Arc<dyn FriendRequestListener>);
    fn remove_friend_request_listener(&self, listener: &Arc<dyn FriendRequestListener>);

    /// Remove every registered listener.
    fn remove_all_listeners(&self);

    // -----------------------------------------------------------------
    // Messages
    // -----------------------------------------------------------------

    /// Create a message builder addressed to `recipient`.  Pass `None` to
    /// create a broadcast message.
    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder>;

    /// Retrieve a single conversation by the other party's `Id`.
    fn get_conversation(&self, id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Conversation>>>>;

    /// Retrieve all conversations.
    fn get_conversations(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Conversation>>>>;

    /// Delete a conversation and its messages.
    fn remove_conversation(&self, id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Delete multiple conversations.
    fn remove_conversations(&self, ids: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Retrieve messages from `conversation_id`, going back up to `until`
    /// with at most `limit` rows skipping `offset`.
    fn get_messages(
        &self,
        conversation_id: &Id,
        until:           Option<i64>,
        limit:           usize,
        offset:          usize,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>>;

    /// Retrieve messages from a time range `[begin, end)` (milliseconds).
    fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin:           i64,
        end:             i64,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>>;

    /// Delete a single message by its local ID.
    fn remove_message(&self, message_id: i64) -> BoxFuture<'_, Result<()>>;

    /// Delete multiple messages by their local IDs.
    fn remove_messages_by_ids(&self, message_ids: &[i64]) -> BoxFuture<'_, Result<()>>;

    /// Delete all messages within a conversation.
    fn remove_messages_in_conversation(&self, conversation_id: &Id) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------

    /// List all known device sessions for the authenticated user.
    fn get_sessions(&self) -> BoxFuture<'_, Result<Vec<SessionInfo>>>;

    /// Revoke (log out) the session identified by `device_id`.
    fn revoke_session(&self, device_id: &Id) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Friends
    // -----------------------------------------------------------------

    /// Send a friend request to `user_id` with an optional greeting.
    fn friend_request(&self, user_id: Id, hello: Option<String>) -> BoxFuture<'_, Result<()>>;

    /// Accept an incoming friend request from `user_id`.
    fn accept_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Look up a specific friend request by the initiator's `Id`.
    fn get_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn FriendRequest>>>>;

    /// Retrieve all pending / received friend requests.
    fn get_friend_requests(&self) -> BoxFuture<'_, Result<Vec<Box<dyn FriendRequest>>>>;

    /// Delete a friend request by user `Id`.
    fn remove_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Delete multiple friend requests.
    fn remove_friend_requests(&self, user_ids: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Delete all friend requests.
    fn clear_friend_requests(&self) -> BoxFuture<'_, Result<()>>;

    /// Add a contact as a friend once a shared session key has been established.
    fn add_friend(
        &self,
        user_id:     Id,
        session_key: Vec<u8>,
        remark:      Option<String>,
    ) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Channels
    // -----------------------------------------------------------------

    /// Create a new channel.
    fn create_channel(
        &self,
        permission:  Permission,
        name:        String,
        notice:      Option<String>,
        announcement: Option<String>,
    ) -> BoxFuture<'_, Result<Box<dyn Channel>>>;

    /// Delete a channel (owner only).
    fn remove_channel(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Join a channel using an invite ticket.
    fn join_channel(&self, ticket: InviteTicket) -> BoxFuture<'_, Result<Box<dyn Channel>>>;

    /// Leave a channel.
    fn leave_channel(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Create an invite ticket for a channel.  If `invitee` is `None` the ticket is a bearer ticket.
    fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee:    Option<Id>,
    ) -> BoxFuture<'_, Result<InviteTicket>>;

    /// Transfer channel ownership to another user.
    fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner:  Id,
    ) -> BoxFuture<'_, Result<()>>;

    /// Rotate the channel session key, optionally supplying a pre-generated keypair.
    fn rotate_channel_session_key(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Update channel metadata.
    fn update_channel_info(&self, channel: &dyn Channel) -> BoxFuture<'_, Result<()>>;

    /// Update the roles of a set of channel members.
    fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members:    &[Id],
        role:       crate::messaging::channel::Role,
    ) -> BoxFuture<'_, Result<()>>;

    /// Ban a set of channel members.
    fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Unban a set of channel members.
    fn unban_channel_members(&self, channel_id: &Id, members: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Remove a set of channel members.
    fn remove_channel_members(&self, channel_id: &Id, members: &[Id]) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Contacts
    // -----------------------------------------------------------------

    /// Look up a contact by `Id`.
    fn get_contact(&self, id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Contact>>>>;

    /// Retrieve all contacts.
    fn get_contacts(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Contact>>>>;

    /// Persist contact updates (remark, tags, muted, blocked …).
    fn update_contact(&self, contact: &dyn Contact) -> BoxFuture<'_, Result<()>>;

    /// Delete a contact.
    fn remove_contact(&self, id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Delete multiple contacts.
    fn remove_contacts(&self, ids: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Delete all contacts.
    fn clear_contacts(&self) -> BoxFuture<'_, Result<()>>;
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing a [`MessagingClient`].
pub struct MessagingClientBuilder {
    service_peer_id:  Option<Id>,
    service_endpoint: Option<url::Url>,
    user_key:         Option<crate::signature::KeyPair>,
    device_key:       Option<crate::signature::KeyPair>,
    data_dir:         Option<std::path::PathBuf>,

    connection_listener:     Option<Arc<dyn ConnectionListener>>,
    message_listener:        Option<Arc<dyn MessageListener>>,
    channel_listener:        Option<Arc<dyn ChannelListener>>,
    contact_listener:        Option<Arc<dyn ContactListener>>,
    session_listener:        Option<Arc<dyn SessionListener>>,
    friend_request_listener: Option<Arc<dyn FriendRequestListener>>,
}

impl Default for MessagingClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingClientBuilder {
    pub fn new() -> Self {
        Self {
            service_peer_id:  None,
            service_endpoint: None,
            user_key:         None,
            device_key:       None,
            data_dir:         None,
            connection_listener:     None,
            message_listener:        None,
            channel_listener:        None,
            contact_listener:        None,
            session_listener:        None,
            friend_request_listener: None,
        }
    }

    pub fn service_peer_id(mut self, id: Id) -> Self {
        self.service_peer_id = Some(id); self
    }

    pub fn service_endpoint(mut self, url: url::Url) -> Self {
        self.service_endpoint = Some(url); self
    }

    pub fn user_key(mut self, kp: crate::signature::KeyPair) -> Self {
        self.user_key = Some(kp); self
    }

    pub fn device_key(mut self, kp: crate::signature::KeyPair) -> Self {
        self.device_key = Some(kp); self
    }

    pub fn data_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.data_dir = Some(dir); self
    }

    pub fn connection_listener(mut self, l: Arc<dyn ConnectionListener>) -> Self {
        self.connection_listener = Some(l); self
    }

    pub fn message_listener(mut self, l: Arc<dyn MessageListener>) -> Self {
        self.message_listener = Some(l); self
    }

    pub fn channel_listener(mut self, l: Arc<dyn ChannelListener>) -> Self {
        self.channel_listener = Some(l); self
    }

    pub fn contact_listener(mut self, l: Arc<dyn ContactListener>) -> Self {
        self.contact_listener = Some(l); self
    }

    pub fn session_listener(mut self, l: Arc<dyn SessionListener>) -> Self {
        self.session_listener = Some(l); self
    }

    pub fn friend_request_listener(mut self, l: Arc<dyn FriendRequestListener>) -> Self {
        self.friend_request_listener = Some(l); self
    }
}
