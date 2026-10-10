use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{Id, core::logger};
use crate::errors::{
    Result,
    StateError,
    ArgumentError
};

use crate::messaging::{
    channel::{Channel, Permission, Role},
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::Contact,
    contact_listener::ContactListener,
    conversation::Conversation,
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::{ContentDisposition, Message, MessageBuilder},
    message_listener::MessageListener,
    options::Options,
    session_info::SessionInfo,
    session_listener::SessionListener,
    MessagingClient,
    verticle::{self, VerticleClient, VerticleOptions},
    ClientConnectionListener
};

pub(crate) use super::internal::{
    PhotonContact,
};

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
    verticle: Mutex<Option<Arc<VerticleClient>>>,
}

impl Client {
    pub fn new(options: Options) -> Self {
        if log::max_level() == log::LevelFilter::Off {
            logger::setup(options.log_level(), options.log_file());
            if options.log_console() {
                logger::enable_console_output();
            } else {
                logger::disable_console_output();
            }
        }

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
            verticle: Mutex::new(None),
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

    pub fn peer_endpoint(&self) -> &url::Url {
        self.options.peer_endpoint()
    }

    pub fn service_peer_id(&self) -> &Id {
        self.peer_id()
    }

    pub fn service_endpoint(&self) -> Option<&str> {
        Some(self.peer_endpoint().as_str())
    }

    pub fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    pub async fn start(&self) -> Result<()> {
        if self.running.swap(true, Ordering::Acquire) {
            return Ok(());
        }

        let connection_listener = Arc::new(ClientConnectionListener::new(
            self.connection_listener.clone(),
            self.connected.clone(),
            self.ready.clone(),
        ));

        let options = VerticleOptions {
            options: self.options.clone(),
            connection_listener,
            message_listener: self.message_listener.clone(),
            channel_listener: self.channel_listener.clone(),
            contact_listener: self.contact_listener.clone(),
            session_listener: self.session_listener.clone(),
            friend_request_listener: self.friend_request_listener.clone(),
        };

        let verticle = match verticle::deploy(options) {
            Ok(verticle) => verticle,
            Err(e) => {
                self.running.store(false, Ordering::Release);
                return Err(StateError::new(&format!("{e}")));
            }
        };

        if let Err(error) = verticle.start().await {
            if let Err(stop_error) = verticle.stop().await {
                log::error!("Stopping messaging verticle failed: {stop_error}");
            }
            self.running.store(false, Ordering::Release);
            return Err(error);
        }
        *self.verticle.lock().unwrap() = Some(Arc::new(verticle));

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let verticle = self.verticle.lock().unwrap().take();
        let result = match verticle {
            Some(v) => v.stop().await,
            _ => Ok(()),
        };
        self.running.store(false, Ordering::Release);
        self.connected.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
        result
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn verticle(&self) -> Result<Arc<VerticleClient>> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected"));
        }
        let verticle_guard = self.verticle.lock().unwrap();
        let Some(v) = verticle_guard.as_ref() else {
            return Err(StateError::new("Messaging verticle is not deployed"));
        };
        Ok(v.clone())
    }

    pub fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(ComposedMessageBuilder::new(
            recipient,
            self.verticle.lock().unwrap().as_ref().map(|v| v.sender()),
            self.message_listener.clone(),
        ))
    }

    pub async fn get_conversation(&self, id: &Id) -> Result<Option<Box<dyn Conversation>>> {
        self.verticle()?.get_conversation(*id).await
    }

    pub async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
        self.verticle()?.get_conversations().await
    }

    pub async fn remove_conversation(&self, id: &Id) -> Result<()> {
        self.verticle()?.remove_conversation(*id).await
    }

    pub async fn remove_conversations(&self, ids: &[Id]) -> Result<()> {
        self.verticle()?.remove_conversations(ids.to_vec()).await
    }

    pub async fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.verticle()?
            .get_messages(*conversation_id, until, limit, offset)
            .await
    }

    pub async fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.verticle()?
            .get_messages_in_range(*conversation_id, begin, end)
            .await
    }

    pub async fn remove_message(&self, message_id: i64) -> Result<()> {
        self.verticle()?.remove_message(message_id).await
    }

    pub async fn remove_messages_by_ids(&self, message_ids: &[i64]) -> Result<()> {
        self.verticle()?.remove_messages_by_ids(message_ids.to_vec()).await
    }

    pub async fn remove_messages_in_conversation(&self, conversation_id: &Id) -> Result<()> {
        self.verticle()?
            .remove_messages_in_conversation(*conversation_id)
            .await
    }

    pub async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.verticle()?.get_sessions().await
    }

    pub async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        self.verticle()?.revoke_session(*device_id).await
    }

    pub async fn friend_request(
        &self,
        user_id: Id,
        hello: Option<String>
    ) -> Result<()> {
        self.verticle()?.friend_request(user_id, hello).await
    }

    pub async fn accept_friend_request(
        &self,
        user_id: Id,
    ) -> Result<()> {
        self.verticle()?.friend_accept(user_id).await
    }

    pub async fn get_friend_request(
        &self,
        user_id: Id,
    ) -> Result<Option<FriendRequest>> {
        self.verticle()?.get_friend_request(user_id).await
    }

    pub async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        self.verticle()?.get_friend_requests().await
    }

    pub async fn remove_friend_request(
        &self,
        user_id: Id
    ) -> Result<()> {
        self.verticle()?.remove_friend_request(user_id).await
    }

    pub async fn remove_friend_requests(
        &self,
        user_ids: &[Id]
    ) -> Result<()> {

        self.verticle()?.remove_friend_requests(user_ids.to_vec()).await
    }

    pub async fn clear_friend_requests(&self) -> Result<()> {
        self.verticle()?.clear_friend_requests().await
    }

    pub async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.verticle()?.add_friend(user_id, session_key, remark).await
    }

    pub async fn block_user(
        &self,
        user_id: Id,
    ) -> Result<Box<dyn Contact>> {
        self.verticle()?.block_user(user_id).await
    }

    /// Creates a new channel.
    pub async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        self.verticle()?
            .create_channel(permission, name, notice, announcement)
            .await
    }

    /// Removes a channel (owner only).
    pub async fn remove_channel(
        &self,
        channel_id: &Id
    ) -> Result<()> {
        self.verticle()?.remove_channel(*channel_id).await
    }

    /// Joins a channel using an invite ticket.
    pub async fn join_channel(
        &self,
        ticket: InviteTicket
    ) -> Result<Box<dyn Channel>> {
        self.verticle()?.join_channel(ticket).await
    }

    /// Leaves a channel.
    pub async fn leave_channel(
        &self,
        channel_id: &Id
    ) -> Result<()> {
        self.verticle()?.leave_channel(*channel_id).await
    }

    /// Creates an invite ticket for a channel.
    pub async fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        self.verticle()?.create_invite_ticket(*channel_id, invitee).await
    }

    /// Transfers channel ownership to another user.
    pub async fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner: Id,
    ) -> Result<()> {
        self.verticle()?
            .transfer_channel_ownership(*channel_id, new_owner)
            .await
    }

    /// Rotates the channel session key.
    pub async fn rotate_channel_session_key(
        &self,
        channel_id: &Id
    ) -> Result<()> {
        self.verticle()?
            .rotate_channel_session_key(*channel_id)
            .await
    }

    /// Updates channel metadata.
    pub async fn update_channel_info(
        &self,
        channel: &dyn Channel
    ) -> Result<()> {
        self.verticle()?
            .update_channel_info(channel.edit_channel().build())
            .await
    }

    pub async fn set_channel_members_role(
        &self,
        channel_id: &Id,
        member: &[Id],
        role: &str
    ) -> Result<()> {
        let role = match role {
            "Owner" => Role::Owner,
            "Moderator" => Role::Moderator,
            "Member" => Role::Member,
            "Banned" => Role::Banned,
            _ => return Err(ArgumentError::new(format!("Invalid channel role: {role}"))),
        };
        self.verticle()?
            .set_channel_members_role(*channel_id, member.to_vec(), role)
            .await
    }

    pub async fn ban_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id]
    ) -> Result<()> {
        self.verticle()?
            .ban_channel_members(*channel_id, members.to_vec())
            .await
    }

    pub async fn unban_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id]
    ) -> Result<()> {
        self.verticle()?
            .unban_channel_members(*channel_id, members.to_vec())
            .await
    }

    pub async fn remove_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id]
    ) -> Result<()> {
        self.verticle()?
            .remove_channel_members(*channel_id, members.to_vec())
            .await
    }

    pub async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        self.verticle()?
            .get_contact(*id)
            .await
            .map(|v| v.map(|v|Box::new(v) as Box<dyn Contact>))
    }

    pub async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        self.verticle()?
            .get_contacts()
            .await
            .map(|v| v.into_iter().map(|v| Box::new(v) as Box<dyn Contact>)
            .collect())
    }

    pub async fn remove_contact(&self, id: &Id) -> Result<()> {
        self.verticle()?.remove_contact(*id).await
    }

    pub async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        self.verticle()?
            .update_contact(PhotonContact::from(contact))
            .await
    }

    pub async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        self.verticle()?.remove_contacts(ids.to_vec()).await
    }

    pub async fn clear_contacts(&self) -> Result<()> {
        self.verticle()?.clear_contacts().await
    }
}

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

    async fn get_conversation(
        &self,
        id: &Id
    ) -> Result<Option<Box<dyn Conversation>>> {
        self.get_conversation(id).await
    }

    async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
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
        self.get_messages(conversation_id, until, limit, offset).await
    }

    async fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.get_messages_in_range(conversation_id, begin, end).await
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

    async fn get_friend_request(&self,
        user_id: &Id,
    ) -> Result<Option<FriendRequest>> {
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
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.add_friend(*user_id, session_key, remark).await
    }

    async fn block_user(
        &self,
        user_id: &Id
    ) -> Result<Box<dyn Contact>> {
        self.block_user(*user_id).await
    }

    async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        self.create_channel(permission, name, notice, announcement)
            .await
    }

    async fn remove_channel(&self, channel_id: &Id) -> Result<()> {
        self.remove_channel(channel_id).await
    }

    async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<dyn Channel>> {
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

    async fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner: Id,
    ) -> Result<()> {
        self.transfer_channel_ownership(channel_id, new_owner).await
    }

    async fn rotate_channel_session_key(&self, channel_id: &Id) -> Result<()> {
        self.rotate_channel_session_key(channel_id).await
    }

    async fn update_channel_info(&self, channel: &dyn Channel) -> Result<()> {
        self.update_channel_info(channel).await
    }

    async fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
    ) -> Result<()> {
        self.set_channel_members_role(channel_id, members, &role.to_string())
            .await
    }

    async fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.ban_channel_members(channel_id, members).await
    }

    async fn unban_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id],
    ) -> Result<()> {
        self.unban_channel_members(channel_id, members).await
    }

    async fn remove_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id],
    ) -> Result<()> {
        self.remove_channel_members(channel_id, members).await
    }

    async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        self.get_contact(id).await
    }

    async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        self.get_contacts().await
    }

    async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
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

#[allow(dead_code)]
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
            let recipient = self.recipient
                .ok_or_else(|| ArgumentError::new("Message recipient is required"))?;
            let body = self.body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let tx = self.verticle_tx
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;
            let mut headers = HashMap::new();
            for (key, value) in self.headers {
                headers.insert(key, serde_json::Value::String(value));
            }
            if let Some(content_type) = self.content_type {
                headers.insert("Content-Type".into(), serde_json::Value::String(content_type));
            } else if !self.text {
                headers.insert(
                    "Content-Type".into(),
                    serde_json::Value::String(crate::messaging::message::content_type::BINARY.into()),
                );
            }
            if let Some(disposition) = self.content_disposition {
                headers.insert(
                    crate::messaging::message::CONTENT_DISPOSITION_HEADER.into(),
                    serde_json::Value::String(disposition.value()),
                );
            }

            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            tx.send(verticle::Event::ContentMessage {
                recipient,
                headers,
                body,
                text: self.text,
                complete: reply_tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
            let message = reply_rx
                .await
                .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
                .map_err(StateError::new)?;
            self.message_listener.on_sent(message.as_ref());
            Ok(message)
        })
    }
}
