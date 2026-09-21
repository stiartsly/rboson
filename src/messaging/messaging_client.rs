use std::path::Path;
use std::sync::Arc;

use crate::messaging::{
    channel::{Channel, Permission, Role},
    channel_listener::ChannelListener,
    client::BoxFuture,
    connection_listener::ConnectionListener,
    contact::Contact,
    contact_listener::ContactListener,
    conversation::Conversation,
    errors::Result,
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::{Message, MessageBuilder},
    message_listener::MessageListener,
    session_info::SessionInfo,
    session_listener::SessionListener,
};
use crate::Id;

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// The primary interface for the Boson Messaging Client.
pub trait MessagingClient: Send + Sync {
    /// Retrieves the identifier of the local user.
    fn user_id(&self) -> &Id;

    /// Retrieves the identifier of the current device.
    fn device_id(&self) -> &Id;

    /// Retrieves the identifier of the messaging service peer.
    fn service_peer_id(&self) -> &Id;

    /// Retrieves the endpoint of the messaging service.
    fn service_endpoint(&self) -> Option<&str>;

    /// Retrieves the data directory used by the client.
    fn data_dir(&self) -> &Path;

    /// Adds a listener for connection state changes.
    fn add_connection_listener(&self, listener: Arc<dyn ConnectionListener>);

    /// Removes a previously added connection state listener.
    fn remove_connection_listener(&self, listener: &Arc<dyn ConnectionListener>);

    /// Adds a listener for message-related events.
    fn add_message_listener(&self, listener: Arc<dyn MessageListener>);

    /// Removes a previously added message listener.
    fn remove_message_listener(&self, listener: &Arc<dyn MessageListener>);

    /// Adds a listener for channel-related events.
    fn add_channel_listener(&self, listener: Arc<dyn ChannelListener>);

    /// Removes a previously added channel listener.
    fn remove_channel_listener(&self, listener: &Arc<dyn ChannelListener>);

    /// Adds a listener for contact-related events.
    fn add_contact_listener(&self, listener: Arc<dyn ContactListener>);

    /// Removes a previously added contact listener.
    fn remove_contact_listener(&self, listener: &Arc<dyn ContactListener>);

    /// Adds a session listener to receive session-related events.
    fn add_session_listener(&self, listener: Arc<dyn SessionListener>);

    /// Removes a session listener from the list of listeners.
    fn remove_session_listener(&self, listener: &Arc<dyn SessionListener>);

    /// Adds a listener for friend request events.
    fn add_friend_request_listener(&self, listener: Arc<dyn FriendRequestListener>);

    /// Removes a previously added friend request listener.
    fn remove_friend_request_listener(&self, listener: &Arc<dyn FriendRequestListener>);

    /// Removes all listeners that have been previously registered with this instance.
    fn remove_all_listeners(&self);

    // -----------------------------------------------------------------
    // Message and conversation APIs
    // -----------------------------------------------------------------

    /// Creates a message builder addressed to `recipient` (or broadcast if `None`).
    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder>;

    /// Retrieves a specific conversation by its identifier.
    fn get_conversation(&self, id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Conversation>>>>;

    /// Retrieves all conversations for the local user.
    fn get_conversations(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Conversation>>>>;

    /// Removes a conversation and all its associated messages.
    fn remove_conversation(&self, id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Removes multiple conversations and all their associated messages.
    fn remove_conversations(&self, ids: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Retrieves a list of messages for a specific conversation with pagination support.
    fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>>;

    /// Retrieves a list of messages for a specific conversation within a time range.
    fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>>;

    /// Removes a specific message by its internal identifier.
    fn remove_message(&self, message_id: i64) -> BoxFuture<'_, Result<()>>;

    /// Removes multiple messages by their internal identifiers.
    fn remove_messages_by_ids(&self, message_ids: &[i64]) -> BoxFuture<'_, Result<()>>;

    /// Deletes all messages within a conversation.
    fn remove_messages_in_conversation(&self, conversation_id: &Id) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------

    /// Lists all known device sessions for the authenticated user.
    fn get_sessions(&self) -> BoxFuture<'_, Result<Vec<SessionInfo>>>;

    /// Revokes (logs out) the session identified by `device_id`.
    fn revoke_session(&self, device_id: &Id) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Friends
    // -----------------------------------------------------------------

    /// Sends a friend request to `user_id` with an optional greeting.
    fn friend_request(&self, user_id: Id, hello: Option<String>) -> BoxFuture<'_, Result<()>>;

    /// Accepts an incoming friend request from `user_id`.
    fn accept_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Retrieves a specific friend request by the initiator's `Id`.
    fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> BoxFuture<'_, Result<Option<Box<dyn FriendRequest>>>>;

    /// Retrieves all pending / received friend requests.
    fn get_friend_requests(&self) -> BoxFuture<'_, Result<Vec<Box<dyn FriendRequest>>>>;

    /// Removes a friend request by user `Id`.
    fn remove_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Removes multiple friend requests.
    fn remove_friend_requests(&self, user_ids: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Clears all friend requests.
    fn clear_friend_requests(&self) -> BoxFuture<'_, Result<()>>;

    /// Adds a contact as a friend once a shared session key has been established.
    fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Channels
    // -----------------------------------------------------------------

    /// Creates a new channel.
    fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: Option<String>,
    ) -> BoxFuture<'_, Result<Box<dyn Channel>>>;

    /// Removes a channel (owner only).
    fn remove_channel(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Joins a channel using an invite ticket.
    fn join_channel(&self, ticket: InviteTicket) -> BoxFuture<'_, Result<Box<dyn Channel>>>;

    /// Leaves a channel.
    fn leave_channel(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Creates an invite ticket for a channel.
    fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> BoxFuture<'_, Result<InviteTicket>>;

    /// Transfers channel ownership to another user.
    fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner: Id,
    ) -> BoxFuture<'_, Result<()>>;

    /// Rotates the channel session key.
    fn rotate_channel_session_key(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Updates channel metadata.
    fn update_channel_info(&self, channel: &dyn Channel) -> BoxFuture<'_, Result<()>>;

    /// Updates the roles of a set of channel members.
    fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
    ) -> BoxFuture<'_, Result<()>>;

    /// Bans a set of channel members.
    fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Unbans a set of channel members.
    fn unban_channel_members(&self, channel_id: &Id, members: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Removes a set of channel members.
    fn remove_channel_members(&self, channel_id: &Id, members: &[Id]) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Contacts
    // -----------------------------------------------------------------

    /// Looks up a contact by `Id`.
    fn get_contact(&self, id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Contact>>>>;

    /// Retrieves all contacts.
    fn get_contacts(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Contact>>>>;

    /// Persists contact updates (remark, tags, muted, blocked …).
    fn update_contact(&self, contact: &dyn Contact) -> BoxFuture<'_, Result<()>>;

    /// Removes a contact by `Id`.
    fn remove_contact(&self, id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Removes multiple contacts.
    fn remove_contacts(&self, ids: &[Id]) -> BoxFuture<'_, Result<()>>;

    /// Clears all contacts.
    fn clear_contacts(&self) -> BoxFuture<'_, Result<()>>;
}
