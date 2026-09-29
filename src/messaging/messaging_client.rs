use std::path::Path;
use std::future::Future;

use crate::errors::{Result};
use crate::messaging::{
    channel::{Channel, Permission, Role},
    contact::Contact,
    conversation::Conversation,
    friend_request::FriendRequest,
    invite_ticket::InviteTicket,
    message::{Message, MessageBuilder},
    session_info::SessionInfo,
};
use crate::Id;

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// The primary interface for the Boson Messaging Client.
pub trait MessagingClient {
    /// Retrieves the identifier of the local user.
    fn user_id(&self) -> &Id;

    /// Retrieves the identifier of the current device.
    fn device_id(&self) -> &Id;

    /// Retrieves the identifier of the director node, if configured.
    fn director_node_id(&self) -> Option<&Id> {
        None
    }

    /// Retrieves the endpoint of the director service, if configured.
    fn director_endpoint(&self) -> Option<&str> {
        None
    }

    /// Retrieves the identifier of the messaging service peer.
    fn service_peer_id(&self) -> &Id;

    /// Retrieves the endpoint of the messaging service.
    fn service_endpoint(&self) -> Option<&str>;

    /// Retrieves the data directory used by the client.
    fn data_dir(&self) -> &Path;

    // -----------------------------------------------------------------
    // Start and stop, status check
    // -----------------------------------------------------------------

    /// Starts the messaging client, initiating the connection and synchronization process.
    fn start(&self) -> impl Future<Output = Result<()>>;

    /// Stops the messaging client and releases all associated resources.
    fn stop(&self) -> impl Future<Output = Result<()>>;

    /// Checks if the messaging client is currently running.
    fn is_running(&self) -> bool;

    /// Checks if the messaging client is currently connected to the messaging service.
    fn is_connected(&self) -> bool;

    /// Checks if the messaging client is ready to send and receive messages.
    fn is_ready(&self) -> bool;

    /// Retrieves the current connection status of the messaging client.
    fn connection_status(&self) -> &str {
        if self.is_ready() {
            "Connected (Ready)"
        } else if self.is_connected() {
            "Connected"
        } else if self.is_running() {
            "Connecting"
        } else {
            "Disconnected"
        }
    }

    // -----------------------------------------------------------------
    // Message and conversation APIs
    // -----------------------------------------------------------------

    /// Creates a message builder addressed to `recipient` (or broadcast if `None`).
    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder>;

    /// Retrieves a specific conversation by its identifier.
    fn get_conversation(
        &self,
        id: &Id
    ) -> impl Future<Output = Result<Option<Box<dyn Conversation>>>>;

    /// Retrieves all conversations for the local user.
    fn get_conversations(&self) -> impl Future<Output = Result<Vec<Box<dyn Conversation>>>>;

    /// Removes a conversation and all its associated messages.
    fn remove_conversation(&self, id: &Id) -> impl Future<Output = Result<()>>;

    /// Removes multiple conversations and all their associated messages.
    fn remove_conversations(
        &self,
        ids: &[Id]
    ) -> impl Future<Output = Result<()>>;

    /// Retrieves a list of messages for a specific conversation with pagination support.
    fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Box<dyn Message>>>>;

    /// Retrieves a list of messages for a specific conversation within a time range.
    fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> impl Future<Output = Result<Vec<Box<dyn Message>>>>;

    /// Removes a specific message by its internal identifier.
    fn remove_message(
        &self,
        message_id: i64
    ) -> impl Future<Output = Result<()>>;

    /// Removes multiple messages by their internal identifiers.
    fn remove_messages_by_ids(
        &self,
        message_ids: &[i64]
    ) -> impl Future<Output = Result<()>>;

    /// Deletes all messages within a conversation.
    fn remove_messages_in_conversation(
        &self,
        conversation_id: &Id
    ) -> impl Future<Output = Result<()>>;

    // -----------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------

    /// Lists all known device sessions for the authenticated user.
    fn get_sessions(
        &self
    ) -> impl Future<Output = Result<Vec<SessionInfo>>>;

    /// Revokes (logs out) the session identified by `device_id`.
    fn revoke_session(
        &self,
        device_id: &Id
    ) -> impl Future<Output = Result<()>>;

    // -----------------------------------------------------------------
    // Friends
    // -----------------------------------------------------------------

    /// Sends a friend request to `user_id` with an optional greeting.
    fn friend_request(
        &self,
        user_id: Id,
        hello: Option<String>
    ) -> impl Future<Output = Result<()>>;

    /// Accepts an incoming friend request from `user_id`.
    fn accept_friend_request(
        &self,
        user_id: &Id
    ) -> impl Future<Output = Result<()>>;

    /// Retrieves a specific friend request by the initiator's `Id`.
    fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> impl Future<Output = Result<Option<Box<dyn FriendRequest>>>>;

    /// Retrieves all pending / received friend requests.
    fn get_friend_requests(
        &self
    ) -> impl Future<Output = Result<Vec<Box<dyn FriendRequest>>>>;

    /// Removes a friend request by user `Id`.
    fn remove_friend_request(
        &self,
        user_id: &Id
    ) -> impl Future<Output = Result<()>>;

    /// Removes multiple friend requests.
    fn remove_friend_requests(
        &self,
        user_ids: &[Id]
    ) -> impl Future<Output = Result<()>>;

    /// Clears all friend requests.
    fn clear_friend_requests(
        &self
    ) -> impl Future<Output = Result<()>>;

    /// Adds a contact as a friend once a shared session key has been established.
    fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> impl Future<Output = Result<()>>;

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
    ) -> impl Future<Output = Result<Box<dyn Channel>>>;

    /// Removes a channel (owner only).
    fn remove_channel(
        &self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    /// Joins a channel using an invite ticket.
    fn join_channel(
        &self,
        ticket: InviteTicket
    ) -> impl Future<Output = Result<Box<dyn Channel>>>;

    /// Leaves a channel.
    fn leave_channel(
        &self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    /// Creates an invite ticket for a channel.
    fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> impl Future<Output = Result<InviteTicket>>;

    /// Transfers channel ownership to another user.
    fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner: Id,
    ) -> impl Future<Output = Result<()>>;

    /// Rotates the channel session key.
    fn rotate_channel_session_key(
        &self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    /// Updates channel metadata.
    fn update_channel_info(
        &self,
        channel: &dyn Channel
    ) -> impl Future<Output = Result<()>>;

    /// Updates the roles of a set of channel members.
    fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
    ) -> impl Future<Output = Result<()>>;

    /// Bans a set of channel members.
    fn ban_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id]
    ) -> impl Future<Output = Result<()>>;

    /// Unbans a set of channel members.
    fn unban_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id]
    ) -> impl Future<Output = Result<()>>;

    /// Removes a set of channel members.
    fn remove_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id]
    ) -> impl Future<Output = Result<()>>;

    // -----------------------------------------------------------------
    // Contacts
    // -----------------------------------------------------------------

    /// Looks up a contact by `Id`.
    fn get_contact(
        &self,
        id: &Id
    ) -> impl Future<Output = Result<Option<Box<dyn Contact>>>>;

    /// Retrieves all contacts.
    fn get_contacts(&self) -> impl Future<Output = Result<Vec<Box<dyn Contact>>>>;

    /// Persists contact updates (remark, tags, muted, blocked …).
    fn update_contact(
        &self,
        contact: &dyn Contact
    ) -> impl Future<Output = Result<()>>;

    /// Removes a contact by `Id`.
    fn remove_contact(&self, id: &Id) -> impl Future<Output = Result<()>>;

    /// Removes multiple contacts.
    fn remove_contacts(&self, ids: &[Id]) -> impl Future<Output = Result<()>>;

    /// Clears all contacts.
    fn clear_contacts(&self) -> impl Future<Output = Result<()>>;
}
