use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "messaging_client.rs"]
pub mod messaging_client;

pub use messaging_client::MessagingClient;

use super::verticle::{self, VerticleClient};

use crate::errors::{Result, NotImplemented, StateError, ArgumentError};
use crate::messaging::{
    channel::{Channel, Permission, Role},
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::{Contact, ContactType},
    contact_listener::ContactListener,
    conversation::Conversation,
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::{Content, ContentDisposition, Message, MessageBuilder},
    message_listener::MessageListener,
    options::Options,
    session_info::SessionInfo,
    session_listener::SessionListener,
};

use super::internal::{
    PhotonContact,
};
use crate::{core::logger, Id};

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// Concrete implementation of a [`FriendRequest`].
#[derive(Debug, Clone)]
pub struct PhotonFriendRequest {
    pub user_id: Id,
    pub initiator_id: Id,
    pub hello: Option<String>,
    pub accepted: bool,
    pub expired: bool,
    pub created_at: SystemTime,
    pub accepted_at: Option<SystemTime>,
    pub updated_at: SystemTime,
}

pub(crate) struct PhotonMessage {
    pub(crate) id: Id,
    pub(crate) recipient: Id,
    pub(crate) from: Option<Id>,
    pub(crate) created_at: SystemTime,
    pub(crate) received_at: Option<SystemTime>,
    pub(crate) sent_at: Option<SystemTime>,
    pub(crate) payload: Vec<u8>,
    pub(crate) content: Content,
}

impl Message for PhotonMessage {
    fn id(&self) -> &Id { &self.id }
    fn rid(&self) -> i64 { 0 }
    fn conversation_id(&self) -> Option<&Id> {
        self.from.as_ref().or(Some(&self.recipient))
    }
    fn recipient(&self) -> &Id { &self.recipient }
    fn message_type(&self) -> crate::messaging::message::MessageType {
        crate::messaging::message::MessageType::ContentMessage
    }
    fn from(&self) -> Option<&Id> { self.from.as_ref() }
    fn created_at(&self) -> SystemTime { self.created_at }
    fn received_at(&self) -> Option<SystemTime> { self.received_at }
    fn sent_at(&self) -> Option<SystemTime> { self.sent_at }
    fn payload_as_bytes(&self) -> &[u8] { &self.payload }
    fn payload_as_content(&self) -> Option<&Content> { Some(&self.content) }
}

impl FriendRequest for PhotonFriendRequest {
    fn user_id(&self) -> &Id {
        &self.user_id
    }

    fn initiator_id(&self) -> &Id {
        &self.initiator_id
    }

    fn hello(&self) -> Option<&str> {
        self.hello.as_deref()
    }

    fn is_accepted(&self) -> bool {
        self.accepted
    }

    fn is_expired(&self) -> bool {
        self.expired
    }

    fn created_at(&self) -> SystemTime {
        self.created_at
    }

    fn accepted_at(&self) -> Option<SystemTime> {
        self.accepted_at
    }

    fn updated_at(&self) -> SystemTime {
        self.updated_at
    }
}

pub(crate) struct ClientState {
    pub(crate) friend_requests: HashMap<Id, PhotonFriendRequest>,
    pub(crate) contacts: HashMap<Id, PhotonContact>,
}

/// The Boson Messaging Client implementation.
pub struct Client {
    options: Arc<Options>,

    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn FriendRequestListener>,
    verticle: Mutex<Option<VerticleClient>>,
    state: Arc<RwLock<ClientState>>,
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

        let state = ClientState {
            friend_requests: HashMap::new(),
            contacts: HashMap::new(),
        };

        let connection_listener = options.connection_listener();
        let message_listener = options.message_listener();
        let channel_listener = options.channel_listener();
        let contact_listener = options.contact_listener();
        let session_listener = options.session_listener();
        let friend_request_listener = options.friend_request_listener();

        Self {
            options: Arc::new(options),
            connected: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            connection_listener,
            message_listener,
            channel_listener,
            contact_listener,
            session_listener,
            friend_request_listener,
            verticle: Mutex::new(None),
            state: Arc::new(RwLock::new(state)),
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
        let is_running = self.verticle.lock().unwrap().is_some();
        if is_running {
            return Ok(());
        }

        let verticle_client = verticle::deploy(
            self.options.clone(),
            self.connected.clone(),
            self.ready.clone(),
            self.connection_listener.clone(),
            self.message_listener.clone(),
            self.channel_listener.clone(),
            self.contact_listener.clone(),
            self.session_listener.clone(),
            self.friend_request_listener.clone(),
            self.state.clone(),
        )?;
        verticle_client.start().await?;
        *self.verticle.lock().unwrap() = Some(verticle_client);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let verticle = self.verticle.lock().unwrap().take();
        if let Some(mut v) = verticle {
            v.stop().await?;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.verticle
            .lock()
            .unwrap()
            .as_ref()
            .map_or(false, |v| v.is_running())
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn connection_status(&self) -> &str {
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

    pub fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(ComposedMessageBuilder::new(
            recipient,
            self.verticle.lock().unwrap().as_ref().map(|v| v.sender()),
            self.message_listener.clone(),
        ))
    }

    pub async fn friend_request(&self, user_id: Id, hello: Option<String>) -> Result<()> {
        MessagingClient::friend_request(self, user_id, hello).await
    }

    pub async fn accept_friend_request(&self, user_id: &Id) -> Result<()> {
        MessagingClient::accept_friend_request(self, user_id).await
    }

    pub async fn get_friend_request(&self, user_id: &Id) -> Result<Option<Box<dyn FriendRequest>>> {
        <Self as MessagingClient>::get_friend_request(self, user_id).await
    }

    pub async fn get_friend_requests(&self) -> Result<Vec<Box<dyn FriendRequest>>> {
        <Self as MessagingClient>::get_friend_requests(self).await
    }

    pub async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        <Self as MessagingClient>::get_contact(self, id).await
    }

    pub async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        <Self as MessagingClient>::get_contacts(self).await
    }

    pub async fn remove_contact(&self, id: &Id) -> Result<()> {
        <Self as MessagingClient>::remove_contact(self, id).await
    }

    pub async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        <Self as MessagingClient>::remove_friend_request(self, user_id).await
    }

    pub async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        <Self as MessagingClient>::remove_friend_requests(self, user_ids).await
    }

    pub async fn clear_friend_requests(&self) -> Result<()> {
        <Self as MessagingClient>::clear_friend_requests(self).await
    }

    pub async fn add_friend(
        &self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        <Self as MessagingClient>::add_friend(self, user_id, session_key, remark).await
    }

    pub async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        <Self as MessagingClient>::update_contact(self, contact).await
    }

    pub async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        <Self as MessagingClient>::remove_contacts(self, ids).await
    }

    pub async fn clear_contacts(&self) -> Result<()> {
        <Self as MessagingClient>::clear_contacts(self).await
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
        _id: &Id
    ) -> Result<Option<Box<dyn Conversation>>> {
        Err(NotImplemented::new("get_conversation"))
    }

    async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
        //self.unsupported("get_conversations")
        Err(NotImplemented::new("get_conversations"))
    }

    async fn remove_conversation(&self, _id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_conversation"))
    }

    async fn remove_conversations(&self, _ids: &[Id]) -> Result<()> {
        Err(NotImplemented::new("remove_conversations"))
    }

    async fn get_messages(
        &self,
        _conversation_id: &Id,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages"))
    }

    async fn get_messages_in_range(
        &self,
        _conversation_id: &Id,
        _begin: i64,
        _end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages_in_range"))
    }

    async fn remove_message(&self, _message_id: i64) -> Result<()> {
        Err(NotImplemented::new("remove_message"))
    }

    async fn remove_messages_by_ids(&self, _message_ids: &[i64]) -> Result<()> {
        Err(NotImplemented::new("remove_messages_by_ids"))
    }

    async fn remove_messages_in_conversation(&self, _conversation_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_messages_in_conversation"))
    }

    async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        Err(NotImplemented::new("get_sessions"))
    }

    async fn revoke_session(&self, _device_id: &Id) -> Result<()> {
        Err(NotImplemented::new("revoke_session"))
    }

    async fn friend_request(&self, user_id: Id, hello: Option<String>) -> Result<()> {
        if &user_id == self.user_id() {
            return Err(ArgumentError::new("Cannot send friend request to yourself"));
        }

        let verticle_tx = self.verticle.lock().unwrap().as_ref().map(|v| v.sender());
        if let Some(tx) = verticle_tx {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            tx.send(verticle::VerticleEvent::FriendRequest {
                user_id,
                hello: hello.clone().unwrap_or_default(),
                complete: reply_tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
            let _ = reply_rx
                .await
                .map_err(|_| StateError::new("Messaging verticle response channel closed"))?;
        }

        let now = SystemTime::now();
        let req = PhotonFriendRequest {
            user_id,
            initiator_id: *self.user_id(),
            hello: hello.clone(),
            accepted: false,
            expired: false,
            created_at: now,
            accepted_at: None,
            updated_at: now,
        };
        self.state
            .write()
            .unwrap()
            .friend_requests
            .insert(user_id, req);

        Ok(())
    }

    async fn accept_friend_request(&self, user_id: &Id) -> Result<()> {
        let user_id = *user_id;
        let state = self.state.read().unwrap();
        let req = state.friend_requests.get(&user_id).ok_or_else(|| {
            ArgumentError::new(format!("No friend request found for {user_id}"))
        })?;
        if req.initiator_id == *self.user_id() {
            return Err(ArgumentError::new("Cannot accept your own friend request"));
        }
        if req.accepted {
            return Err(ArgumentError::new("Friend request has already been accepted"));
        }
        if req.expired {
            return Err(ArgumentError::new("Friend request has expired"));
        }

        let verticle_tx = self.verticle.lock().unwrap().as_ref().map(|v| v.sender());
        if let Some(tx) = verticle_tx {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            tx.send(verticle::VerticleEvent::FriendAccept {
                user_id,
                complete: reply_tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
            let _ = reply_rx
                .await
                .map_err(|_| StateError::new("Messaging verticle response channel closed"))?;
        }

        let now = SystemTime::now();
        let now_ms = now
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let contact = PhotonContact {
            id: user_id,
            contact_type: ContactType::Friend,
            name: None,
            remark: None,
            tags: None,
            muted: false,
            blocked: false,
            created_at: now_ms,
            updated_at: now_ms,
            revision: 1,
        };

        {
            let mut state = self.state.write().unwrap();
            if let Some(req) = state.friend_requests.get_mut(&user_id) {
                req.accepted = true;
                req.accepted_at = Some(now);
                req.updated_at = now;
            }
            state.contacts.insert(user_id, contact.clone());
        }

        self.contact_listener.on_contact_added(&contact);
        Ok(())
    }

    async fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> Result<Option<Box<dyn FriendRequest>>> {
        let user_id = *user_id;
        let state = self.state.read().unwrap();
        Ok(state
            .friend_requests
            .get(&user_id)
            .cloned()
            .map(|r| Box::new(r) as Box<dyn FriendRequest>))
    }

    async fn get_friend_requests(&self) -> Result<Vec<Box<dyn FriendRequest>>> {
        let state = self.state.read().unwrap();
        Ok(state
            .friend_requests
            .values()
            .cloned()
            .map(|r| Box::new(r) as Box<dyn FriendRequest>)
            .collect())
    }

    async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        let user_id = *user_id;
        self.state.write().unwrap().friend_requests.remove(&user_id);
        Ok(())
    }

    async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        let user_ids = user_ids.to_vec();
        let mut state = self.state.write().unwrap();
        for id in &user_ids {
            state.friend_requests.remove(id);
        }
        Ok(())
    }

    async fn clear_friend_requests(&self) -> Result<()> {
        self.state.write().unwrap().friend_requests.clear();
        Ok(())
    }

    async fn add_friend(
        &self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        if session_key.len() != crate::signature::PrivateKey::BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid friend session key length: {}",
                session_key.len()
            )));
        }
        let verticle_tx = self.verticle.lock().unwrap().as_ref().map(|v| v.sender());
        if let Some(tx) = verticle_tx {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            tx.send(verticle::VerticleEvent::RegisterFriendSession {
                user_id: *user_id,
                session_key,
                complete: reply_tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
            reply_rx
                .await
                .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
                .map_err(StateError::new)?;
        }
        let now = SystemTime::now();
        let now_ms = now
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let contact = PhotonContact {
            id: *user_id,
            contact_type: ContactType::Friend,
            name: None,
            remark,
            tags: None,
            muted: false,
            blocked: false,
            created_at: now_ms,
            updated_at: now_ms,
            revision: 1,
        };
        self.state.write().unwrap().contacts.insert(*user_id, contact.clone());

        self.contact_listener.on_contact_added(&contact);
        Ok(())
    }

    async fn block_user(
        &self,
        _user_id: &Id
    ) -> Result<Box<dyn Contact>> {
        Err(NotImplemented::new("block_user"))
    }

    async fn create_channel(
        &self,
        _permission: Permission,
        _name: String,
        _notice: Option<String>,
        _announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        Err(NotImplemented::new("create_channel"))
    }

    async fn remove_channel(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_channel"))
    }

    async fn join_channel(&self, _ticket: InviteTicket) -> Result<Box<dyn Channel>> {
        Err(NotImplemented::new("join_channel"))
    }

    async fn leave_channel(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("leave_channel"))
    }

    async fn create_invite_ticket(
        &self,
        _channel_id: &Id,
        _invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        Err(NotImplemented::new("create_invite_ticket"))
    }

    async fn transfer_channel_ownership(
        &self,
        _channel_id: &Id,
        _new_owner: Id,
    ) -> Result<()> {
        Err(NotImplemented::new("transfer_channel_ownership"))
    }

    async fn rotate_channel_session_key(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("rotate_channel_session_key"))
    }

    async fn update_channel_info(&self, _channel: &dyn Channel) -> Result<()> {
        Err(NotImplemented::new("update_channel_info"))
    }

    async fn set_channel_members_role(
        &self,
        _channel_id: &Id,
        _members: &[Id],
        _role: Role,
    ) -> Result<()> {
        Err(NotImplemented::new("set_channel_members_role"))
    }

    async fn ban_channel_members(&self, _channel_id: &Id, _members: &[Id]) -> Result<()> {
        Err(NotImplemented::new("ban_channel_members"))
    }

    async fn unban_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> Result<()> {
        Err(NotImplemented::new("unban_channel_members"))
    }

    async fn remove_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> Result<()> {
        Err(NotImplemented::new("remove_channel_members"))
    }

    async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        let id = *id;
        let state = self.state.read().unwrap();
        Ok(state
            .contacts
            .get(&id)
            .cloned()
            .map(|c| Box::new(c) as Box<dyn Contact>))
    }

    async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        let state = self.state.read().unwrap();
        Ok(state
            .contacts
            .values()
            .cloned()
            .map(|c| Box::new(c) as Box<dyn Contact>)
            .collect())
    }

    async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        let updated = PhotonContact {
            id: *contact.id(),
            contact_type: contact.contact_type(),
            name: contact.name().map(ToString::to_string),
            remark: contact.remark().map(ToString::to_string),
            tags: contact.tags().map(ToString::to_string),
            muted: contact.is_muted(),
            blocked: contact.is_blocked(),
            created_at: contact.created_at(),
            updated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            revision: contact.revision() + 1,
        };
        {
            self.state
                .write()
                .unwrap()
                .contacts
                .insert(updated.id, updated.clone());
            let boxed: Box<dyn Contact> = Box::new(updated.clone());
            self.contact_listener
                .on_contacts_updated(&[boxed]);
            Ok(())
        }
    }

    async fn remove_contact(&self, id: &Id) -> Result<()> {
        let id = *id;
        let verticle_tx = self.verticle.lock().unwrap().as_ref().map(|v| v.sender());
        if let Some(tx) = verticle_tx {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if tx
                .send(verticle::VerticleEvent::FriendRemove {
                    user_id: id,
                    complete: reply_tx,
                })
                .is_ok()
            {
                let _ = reply_rx.await;
            }
        }

        self.state.write().unwrap().contacts.remove(&id);
        self.contact_listener.on_contacts_removed(&[id]);
        Ok(())
    }

    async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        let ids = ids.to_vec();
        let mut state = self.state.write().unwrap();
        for id in &ids {
            state.contacts.remove(id);
        }
        drop(state);
        self.contact_listener.on_contacts_removed(&ids);
        Ok(())
    }

    async fn clear_contacts(&self) -> Result<()> {
        self.state.write().unwrap().contacts.clear();
        self.contact_listener.on_contacts_cleared();
        Ok(())
    }
}

#[allow(dead_code)]
struct ComposedMessageBuilder {
    recipient: Option<Id>,
    verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::VerticleEvent>>,
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
        verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::VerticleEvent>>,
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
            tx.send(verticle::VerticleEvent::ContentMessage {
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
            self.message_listener.on_sent(&message);
            Ok(Box::new(message) as Box<dyn Message>)
        })
    }
}
