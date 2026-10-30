// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{
    collections::HashMap,
    future::Future,
    cell::Cell,
    path::Path,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

use crate::{
    channel::{Permission, Role, Channel},
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::Contact,
    contact_listener::ContactListener,
    conversation::Conversation,
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::{
        content_type, ContentDisposition, Message, MessageBuilder, CONTENT_DISPOSITION_HEADER,
    },
    message_listener::MessageListener,
    options::Options,
    session_info::SessionInfo,
    session_listener::SessionListener,
    verticle::{self, VerticleClient, VerticleOptions},
    ClientConnectionListener, MessagingClient,
};
use boson_api::errors::{ArgumentError, Result, StateError};
use boson_api::{logger, signature, Id};

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// The Boson Messaging Client implementation.
pub struct Client {
    options: Arc<Options>,

    running: AtomicBool,
    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn FriendRequestListener>,

    verticle: Cell<Option<VerticleClient>>,
    lifecycle: tokio::sync::Mutex<()>,
}

// ---------------------------------------------------------------------------
// Construction & accessors
// ---------------------------------------------------------------------------

impl Client {
    pub fn new(options: Options) -> Self {
        Self::setup_logging(&options);

        let connection_listener = options.connection_listener();
        let message_listener = options.message_listener();
        let channel_listener = options.channel_listener();
        let contact_listener = options.contact_listener();
        let session_listener = options.session_listener();
        let friend_request_listener = options.friend_request_listener();

        Self {
            options: Arc::new(options),
            running: AtomicBool::new(false),
            connected: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            connection_listener,
            message_listener,
            channel_listener,
            contact_listener,
            session_listener,
            friend_request_listener,
            verticle: Cell::new(None),
            lifecycle: tokio::sync::Mutex::new(()),
        }
    }

    /// Installs the process-wide logger, but only if nobody else has yet.
    ///
    /// Kept out of `new` so the global side effect is visible in one place.
    fn setup_logging(options: &Options) {
        if log::max_level() != log::LevelFilter::Off {
            return;
        }
        logger::setup(options.log_level(), options.log_file());
        if options.log_console() {
            logger::enable_console_output();
        } else {
            logger::disable_console_output();
        }
    }

    /// The options this client was built from.
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// The user this client authenticates as.
    pub fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    /// The device this client runs on.
    pub fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    /// The messaging service peer.
    pub fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

    /// The messaging service endpoint.
    pub fn peer_endpoint(&self) -> &url::Url {
        self.options.peer_endpoint()
    }

    /// Directory holding this client's local state.
    pub fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------
impl Client {
    pub async fn start(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        if self.is_running() {
            return Ok(());
        }

        let mut verticle = match verticle::deploy(self.verticle_options()).await {
            Ok(verticle) => verticle,
            Err(error) => {
                self.reset_state();
                return Err(error);
            }
        };

        if let Err(error) = verticle.start().await {
            // The thread is already up; do not let it outlive a client that
            // never started.
            if let Err(stop_error) = verticle.stop().await {
                log::error!("Stopping messaging verticle failed: {stop_error}");
            }
            self.reset_state();
            return Err(error);
        }

        self.verticle.set(Some(verticle));
        self.running.store(true, Ordering::Release);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        let result = match self.take_verticle() {
            Some(mut verticle) => verticle.stop().await,
            _ => Ok(()),
        };
        self.reset_state();
        result
    }

    /// Whether the verticle thread is deployed.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Whether the transport completed the MQTT handshake.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// Whether topics are subscribed and application traffic may flow.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

// ---------------------------------------------------------------------------
// Conversations
// ---------------------------------------------------------------------------

impl Client {
    /// Looks up a single conversation by contact id.
    pub async fn get_conversation(&self, id: &Id) -> Result<Option<Conversation>> {
        self.verticle_client()?.get_conversation(*id).await
    }

    /// Lists every known conversation.
    pub async fn get_conversations(&self) -> Result<Vec<Conversation>> {
        self.verticle_client()?.get_conversations().await
    }

    /// Drops the conversation with `id`. The peer is not notified.
    pub async fn remove_conversation(&self, id: &Id) -> Result<()> {
        self.verticle_client()?.remove_conversation(*id).await
    }

    /// Drops several conversations in one round trip.
    pub async fn remove_conversations(&self, ids: &[Id]) -> Result<()> {
        self.verticle_client()?
            .remove_conversations(ids.to_vec())
            .await
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

impl Client {
    /// Reads a page of history, newest first, ending at `until` when given.
    pub async fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.verticle_client()?
            .get_messages(*conversation_id, until, limit, offset)
            .await
    }

    /// Reads every message with a timestamp in `begin..=end`.
    pub async fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.verticle_client()?
            .get_messages_in_range(*conversation_id, begin, end)
            .await
    }

    /// Removes a single message by id.
    pub async fn remove_message(&self, message_id: i64) -> Result<()> {
        self.verticle_client()?.remove_message(message_id).await
    }

    /// Removes several messages by id in one round trip.
    pub async fn remove_messages_by_ids(&self, message_ids: &[i64]) -> Result<()> {
        self.verticle_client()?
            .remove_messages_by_ids(message_ids.to_vec())
            .await
    }

    /// Clears the whole history of a conversation.
    pub async fn remove_messages_in_conversation(&self, conversation_id: &Id) -> Result<()> {
        self.verticle_client()?
            .remove_messages_in_conversation(*conversation_id)
            .await
    }

    /// Starts composing an outgoing message.
    ///
    /// The builder captures the verticle sender as it is created, so a builder
    /// obtained while the client is stopped can never be sent later.
    pub fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        let sender = self.verticle_ref().map(|verticle| verticle.sender());
        Box::new(ComposedMessageBuilder::new(
            recipient,
            sender,
            self.message_listener.clone(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

impl Client {
    /// Lists the sessions currently authorised for this user.
    pub async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.verticle_client()?.get_sessions().await
    }

    /// Revokes another device's session.
    ///
    /// Revoking your own session is rejected: that is what [`Client::stop`] is
    /// for.
    pub async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        if device_id == self.device_id() {
            return Err(ArgumentError::new("Cannot revoke the current session"));
        }
        self.verticle_client()?
            .revoke_session(*device_id)
            .await
            .map_err(|e| e.into())
    }
}

// ---------------------------------------------------------------------------
// Friend requests
// ---------------------------------------------------------------------------

impl Client {
    /// Sends a friend request with an optional greeting.
    pub async fn friend_request(
        &self,
        user_id: Id,
        hello: Option<impl Into<String>>,
    ) -> Result<()> {
        if &user_id == self.user_id() {
            return Err(ArgumentError::new("Cannot send friend request to yourself"));
        }
        self.verticle_client()?
            .friend_request(user_id, hello.map(Into::into))
            .await
            .map_err(|e| e.into())
    }

    /// Accepts a pending friend request.
    pub async fn accept_friend_request(&self, user_id: Id) -> Result<()> {
        if &user_id == self.user_id() {
            return Err(ArgumentError::new(
                "Cannot accept friend request from yourself",
            ));
        }
        self.verticle_client()?.friend_accept(user_id).await.map_err(|e| e.into())
    }

    /// Looks up a single pending friend request.
    pub async fn get_friend_request(&self, user_id: Id) -> Result<Option<FriendRequest>> {
        self.verticle_client()?.get_friend_request(user_id).await
    }

    /// Lists every pending friend request.
    pub async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        self.verticle_client()?.get_friend_requests().await
    }

    /// Dismisses a pending friend request without accepting it.
    pub async fn remove_friend_request(&self, user_id: Id) -> Result<()> {
        self.verticle_client()?.remove_friend_request(user_id).await
    }

    /// Dismisses several pending friend requests in one round trip.
    pub async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        self.verticle_client()?
            .remove_friend_requests(user_ids.to_vec())
            .await
    }

    /// Dismisses every pending friend request.
    pub async fn clear_friend_requests(&self) -> Result<()> {
        self.verticle_client()?.clear_friend_requests().await
    }

    /// Registers an existing friend together with their session key.
    pub async fn add_friend(
        &self,
        user_id: Id,
        session_key: impl Into<signature::KeyPair>,
        remark: Option<impl Into<String>>,
    ) -> Result<()> {
        let key = session_key.into().private_key().as_ref().to_vec();
        self.verticle_client()?
            .add_friend(user_id, key, remark.map(Into::into))
            .await
    }

    /// Blocks a user, which also stops them from messaging you.
    pub async fn block_user(&self, user_id: Id) -> Result<Contact> {
        self.verticle_client()?.block_user(user_id).await
    }
}

// ---------------------------------------------------------------------------
// Contacts
// ---------------------------------------------------------------------------

impl Client {
    /// Looks up a single contact.
    pub async fn get_contact(&self, id: &Id) -> Result<Option<Contact>> {
        self.verticle_client()?.get_contact(*id).await
    }

    /// Lists every known contact.
    pub async fn get_contacts(&self) -> Result<Vec<Contact>> {
        self.verticle_client()?.get_contacts().await
    }

    /// Overwrites a contact's locally stored metadata.
    pub async fn update_contact(&self, contact: Contact) -> Result<()> {
        self.verticle_client()?.update_contact(contact).await
    }

    /// Removes a single contact; sugar for a one-element [`Client::remove_contacts`].
    pub async fn remove_contact(&self, id: &Id) -> Result<()> {
        self.remove_contacts(&[*id]).await
    }

    /// Removes several contacts in one round trip.
    pub async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        if ids.is_empty() {
            return Err(ArgumentError::new("No contact IDs specified"));
        }
        self.verticle_client()?
            .remove_contacts(ids.to_vec())
            .await
            .map_err(|e| e.into())
    }

    /// Removes every contact.
    pub async fn clear_contacts(&self) -> Result<()> {
        self.verticle_client()?.clear_contacts().await
    }
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

impl Client {
    /// Creates a new channel.
    pub async fn create_channel(
        &self,
        permission: Permission,
        name: impl Into<String>,
        notice: Option<impl Into<String>>,
        announce: bool,
    ) -> Result<Box<Channel>> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(ArgumentError::new("Channel name is required"));
        }

        self.verticle_client()?
            .create_channel(permission, name, notice.map(Into::into), announce)
            .await
            .map_err(|e| e.into())
    }

    /// Removes a channel (owner only).
    pub async fn remove_channel(&self, channel_id: &Id) -> Result<()> {
        self.verticle_client()?.remove_channel(*channel_id).await
    }

    /// Joins a channel using an invite ticket.
    pub async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<Channel>> {
        self.verticle_client()?.join_channel(ticket).await
    }

    /// Leaves a channel.
    pub async fn leave_channel(&self, channel_id: &Id) -> Result<()> {
        self.verticle_client()?.leave_channel(*channel_id).await
    }

    /// Creates an invite ticket for a channel.
    pub async fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        self.verticle_client()?
            .create_invite_ticket(*channel_id, invitee)
            .await
    }

    /// Transfers channel ownership to another user.
    pub async fn transfer_channel_ownership(&self, channel_id: &Id, new_owner: Id) -> Result<()> {
        self.verticle_client()?
            .transfer_channel_ownership(*channel_id, new_owner)
            .await
    }

    /// Rotates the channel session key.
    pub async fn rotate_channel_session_key(&self, channel_id: &Id) -> Result<()> {
        self.verticle_client()?
            .rotate_channel_session_key(*channel_id)
            .await
    }

    /// Updates channel metadata.
    pub async fn update_channel_info(&self, channel: &Box<Channel>) -> Result<()> {
        self.verticle_client()?
            .update_channel_info(channel.edit_channel().build())
            .await
    }

    /// Updates the roles of a set of channel members.
    ///
    /// The role is accepted by name so callers can pass a value read from
    /// config or a UI without depending on [`Role`] itself.
    pub async fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
    ) -> Result<()> {
        self.verticle_client()?
            .set_channel_members_role(*channel_id, members.to_vec(), role)
            .await
    }

    /// Bans a set of channel members.
    pub async fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.verticle_client()?
            .ban_channel_members(*channel_id, members.to_vec())
            .await
    }

    /// Unbans a set of channel members.
    pub async fn unban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.verticle_client()?
            .unban_channel_members(*channel_id, members.to_vec())
            .await
    }

    /// Removes a set of channel members.
    pub async fn remove_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.verticle_client()?
            .remove_channel_members(*channel_id, members.to_vec())
            .await
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

impl Client {
    fn verticle_client(&self) -> Result<&VerticleClient> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected"));
        }
        self.verticle_ref()
            .ok_or_else(|| StateError::new("Messaging verticle is not deployed").into())
    }

    #[allow(unsafe_code)]
    fn verticle_ref(&self) -> Option<&VerticleClient> {
        // SAFETY: `Client` is `!Sync` because of `Cell`, and the slot is only
        // replaced by `start`/`stop`, which are serialized by `lifecycle`.
        unsafe { (*self.verticle.as_ptr()).as_ref() }
    }

    fn take_verticle(&self) -> Option<VerticleClient> {
        self.verticle.take()
    }

    fn reset_state(&self) {
        self.running.store(false, Ordering::Release);
        self.connected.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
    }

    fn verticle_options(&self) -> VerticleOptions {
        let connection_listener = Arc::new(ClientConnectionListener::new(
            self.connection_listener.clone(),
            self.connected.clone(),
            self.ready.clone(),
        ));

        VerticleOptions {
            options: self.options.clone(),
            connection_listener,
            message_listener: self.message_listener.clone(),
            channel_listener: self.channel_listener.clone(),
            contact_listener: self.contact_listener.clone(),
            session_listener: self.session_listener.clone(),
            friend_request_listener: self.friend_request_listener.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Trait delegation
// ---------------------------------------------------------------------------
impl MessagingClient for Client {
    fn user_id(&self) -> &Id {
        self.user_id()
    }

    fn device_id(&self) -> &Id {
        self.device_id()
    }

    fn peer_id(&self) -> &Id {
        self.peer_id()
    }

    fn peer_endpoint(&self) -> &str {
        self.options.peer_endpoint().as_str()
    }

    fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    async fn start(&self) -> Result<()> {
        self.start().await
    }

    async fn stop(&self) -> Result<()> {
        self.stop().await
    }

    fn is_running(&self) -> bool {
        self.is_running()
    }

    fn is_connected(&self) -> bool {
        self.is_connected()
    }

    fn is_ready(&self) -> bool {
        self.is_ready()
    }

    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        self.message(recipient)
    }

    async fn get_conversation(&self, id: &Id) -> Result<Option<Conversation>> {
        self.get_conversation(id).await
    }

    async fn get_conversations(&self) -> Result<Vec<Conversation>> {
        self.get_conversations().await
    }

    async fn remove_conversation(&self, id: &Id) -> Result<()> {
        self.remove_conversation(id).await
    }

    async fn remove_conversations(&self, ids: &[Id]) -> Result<()> {
        self.remove_conversations(ids).await
    }

    async fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.get_messages(conversation_id, until, limit, offset)
            .await
    }

    async fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.get_messages_in_range(conversation_id, begin, end)
            .await
    }

    async fn remove_message(&self, message_id: i64) -> Result<()> {
        self.remove_message(message_id).await
    }

    async fn remove_messages_by_ids(&self, message_ids: &[i64]) -> Result<()> {
        self.remove_messages_by_ids(message_ids).await
    }

    async fn remove_messages_in_conversation(&self, conversation_id: &Id) -> Result<()> {
        self.remove_messages_in_conversation(conversation_id).await
    }

    async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.get_sessions().await
    }

    async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        self.revoke_session(device_id).await
    }

    async fn friend_request(&self, userid: &Id, hello: Option<String>) -> Result<()> {
        self.friend_request(*userid, hello).await
    }

    async fn accept_friend_request(&self, userid: &Id) -> Result<()> {
        self.accept_friend_request(*userid).await
    }

    async fn get_friend_request(&self, user_id: &Id) -> Result<Option<FriendRequest>> {
        self.get_friend_request(*user_id).await
    }

    async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        self.get_friend_requests().await
    }

    async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        self.remove_friend_request(*user_id).await
    }

    async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        self.remove_friend_requests(user_ids).await
    }

    async fn clear_friend_requests(&self) -> Result<()> {
        self.clear_friend_requests().await
    }

    async fn add_friend(
        &self,
        userid: &Id,
        session_key: signature::KeyPair,
        remark: Option<String>,
    ) -> Result<()> {
        self.add_friend(*userid, session_key, remark).await
    }

    async fn block_user(&self, userid: &Id) -> Result<Contact> {
        self.block_user(*userid).await
    }

    async fn create_channel(
        &self,
        perms: Permission,
        name: String,
        notice: Option<String>,
        announce: bool,
    ) -> Result<Box<Channel>> {
        self.create_channel(perms, name, notice, announce).await
    }

    async fn remove_channel(&self, channel_id: &Id) -> Result<()> {
        self.remove_channel(channel_id).await
    }

    async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<Channel>> {
        self.join_channel(ticket).await
    }

    async fn leave_channel(&self, channel_id: &Id) -> Result<()> {
        self.leave_channel(channel_id).await
    }

    async fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        self.create_invite_ticket(channel_id, invitee).await
    }

    async fn transfer_channel_ownership(&self, channel_id: &Id, new_owner: Id) -> Result<()> {
        self.transfer_channel_ownership(channel_id, new_owner).await
    }

    async fn rotate_channel_session_key(&self, channel_id: &Id) -> Result<()> {
        self.rotate_channel_session_key(channel_id).await
    }

    async fn update_channel_info(&self, channel: &Box<Channel>) -> Result<()> {
        self.update_channel_info(channel).await
    }

    async fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
    ) -> Result<()> {
        self.set_channel_members_role(channel_id, members, role)
            .await
    }

    async fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.ban_channel_members(channel_id, members).await
    }

    async fn unban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.unban_channel_members(channel_id, members).await
    }

    async fn remove_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.remove_channel_members(channel_id, members).await
    }

    async fn get_contact(&self, id: &Id) -> Result<Option<Contact>> {
        self.get_contact(id).await
    }

    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        self.get_contacts().await
    }

    async fn update_contact(&self, contact: Contact) -> Result<()> {
        self.update_contact(contact).await
    }

    async fn remove_contact(&self, id: &Id) -> Result<()> {
        self.remove_contact(id).await
    }

    async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        self.remove_contacts(ids).await
    }

    async fn clear_contacts(&self) -> Result<()> {
        self.clear_contacts().await
    }
}

// ---------------------------------------------------------------------------
// Message composition
// ---------------------------------------------------------------------------

/// Collects a message on the caller's side, then hands it to the verticle.
///
/// Nothing here touches the session: the builder only decides *what* to send,
/// and [`Client::message`] captures the sender at construction time.
struct ComposedMessageBuilder {
    recipient: Option<Id>,
    verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::Event>>,
    message_listener: Arc<dyn MessageListener>,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    text: bool,
}

impl ComposedMessageBuilder {
    fn new(
        recipient: Option<Id>,
        verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::Event>>,
        message_listener: Arc<dyn MessageListener>,
    ) -> Self {
        Self {
            recipient,
            verticle_tx,
            message_listener,
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        }
    }
}

impl MessageBuilder for ComposedMessageBuilder {
    fn content_type(mut self: Box<Self>, content_type: &str) -> Box<dyn MessageBuilder> {
        self.content_type = Some(content_type.to_string());
        self
    }

    fn content_disposition(
        mut self: Box<Self>,
        disposition: ContentDisposition,
    ) -> Box<dyn MessageBuilder> {
        self.content_disposition = Some(disposition);
        self
    }

    fn text_body(mut self: Box<Self>, text: &str) -> Box<dyn MessageBuilder> {
        self.body = Some(text.as_bytes().to_vec());
        self.text = true;
        self
    }

    fn binary_body(mut self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilder> {
        self.body = Some(data);
        self.text = false;
        self
    }

    fn header(mut self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilder> {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    fn send(
        self: Box<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Message>>> + Send + 'static>> {
        Box::pin(async move {
            let recipient = self
                .recipient
                .ok_or_else(|| ArgumentError::new("Message recipient is required"))?;
            let body = self
                .body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let sender = self
                .verticle_tx
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;

            let text = self.text;
            let headers = merge_headers(
                self.headers,
                self.content_type,
                self.content_disposition,
                text,
            );

            let (promise, future) = verticle::Promise::pair();
            sender
                .send(verticle::Event::ContentMessage(
                    verticle::WithParams {
                        data: (recipient, headers, body, text),
                        promise
                    },
                ))
                .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;

            let message = future.await.map_err(StateError::new)?;
            self.message_listener.on_sent(message.as_ref());
            Ok(message)
        })
    }
}

/// Folds the typed builder fields into the flat header map the verticle takes.
///
/// A binary body without an explicit [`MessageBuilder::content_type`] gets
/// `application/octet-stream`. Note that an explicit type only counts when it
/// came from `content_type`; a raw [`MessageBuilder::header`] of the same name
/// is overwritten here — that matches the behaviour this method replaced.
fn merge_headers(
    headers: Vec<(String, String)>,
    declared_type: Option<String>,
    disposition: Option<ContentDisposition>,
    text: bool,
) -> HashMap<String, serde_json::Value> {
    let mut merged: HashMap<String, serde_json::Value> = headers
        .into_iter()
        .map(|(key, value)| (key, serde_json::Value::String(value)))
        .collect();

    if let Some(declared_type) = declared_type {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(declared_type),
        );
    } else if !text {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(content_type::BINARY.into()),
        );
    }

    if let Some(disposition) = disposition {
        merged.insert(
            CONTENT_DISPOSITION_HEADER.into(),
            serde_json::Value::String(disposition.value()),
        );
    }

    merged
}
