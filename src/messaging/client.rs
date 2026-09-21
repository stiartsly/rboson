use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};

#[path = "messaging_client.rs"]
pub mod messaging_client;

pub use messaging_client::MessagingClient;

use super::verticle::{self, VerticleClient};

use crate::messaging::{
    channel::{Channel, Permission, Role},
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::Contact,
    contact_listener::ContactListener,
    conversation::Conversation,
    errors::{Error, Result},
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::{ContentDisposition, Message, MessageBuilder},
    message_listener::MessageListener,
    options::Options,
    session_info::SessionInfo,
    session_listener::SessionListener,
};
use crate::Id;

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// A boxed future returned by async methods on [`MessagingClient`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) struct SharedListeners {
    pub(crate) connected: AtomicBool,
    pub(crate) ready: AtomicBool,
    pub(crate) connection_listeners: RwLock<Vec<Arc<dyn ConnectionListener>>>,
    pub(crate) message_listeners: RwLock<Vec<Arc<dyn MessageListener>>>,
    pub(crate) channel_listeners: RwLock<Vec<Arc<dyn ChannelListener>>>,
    pub(crate) contact_listeners: RwLock<Vec<Arc<dyn ContactListener>>>,
    pub(crate) session_listeners: RwLock<Vec<Arc<dyn SessionListener>>>,
    pub(crate) friend_request_listeners: RwLock<Vec<Arc<dyn FriendRequestListener>>>,
}

impl SharedListeners {
    pub(crate) fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            connection_listeners: RwLock::new(Vec::new()),
            message_listeners: RwLock::new(Vec::new()),
            channel_listeners: RwLock::new(Vec::new()),
            contact_listeners: RwLock::new(Vec::new()),
            session_listeners: RwLock::new(Vec::new()),
            friend_request_listeners: RwLock::new(Vec::new()),
        }
    }
}

/// The Boson Messaging Client implementation.
pub struct Client {
    options: Options,
    listeners: Arc<SharedListeners>,
    verticle: Mutex<Option<VerticleClient>>,
}

pub type PhotonMessagingClient = Client;

impl Client {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            listeners: Arc::new(SharedListeners::new()),
            verticle: Mutex::new(None),
        }
    }

    fn unavailable<T>(&self, operation: &str) -> Result<T> {
        let state = if self.is_running() {
            "the updated encrypted RPC/persistence runtime has not been ported"
        } else {
            "the messaging client is not running"
        };
        Err(Error::State(format!("{operation} is unavailable: {state}")))
    }

    fn remove_listener<T: ?Sized>(listeners: &RwLock<Vec<Arc<T>>>, target: &Arc<T>) {
        listeners
            .write()
            .unwrap()
            .retain(|listener| !Arc::ptr_eq(listener, target));
    }

    fn unsupported<'a, T: Send + 'a>(
        &'a self,
        operation: &'static str,
    ) -> BoxFuture<'a, Result<T>> {
        Box::pin(async move { self.unavailable(operation) })
    }

    pub fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub fn service_peer_id(&self) -> &Id {
        self.options.service_peerid()
    }

    pub fn service_endpoint(&self) -> Option<&str> {
        self.options.service_endpoint().map(url::Url::as_str)
    }

    pub fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    pub async fn start(&self) -> Result<()> {
        let is_running = self.verticle.lock().unwrap().is_some();
        if is_running {
            return Ok(());
        }

        let verticle_client = verticle::deploy(self.options.clone(), self.listeners.clone())?;
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
        self.listeners.connected.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        self.listeners.ready.load(Ordering::Acquire)
    }

    pub fn add_connection_listener(&self, listener: Arc<dyn ConnectionListener>) {
        self.listeners
            .connection_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_connection_listener(&self, listener: &Arc<dyn ConnectionListener>) {
        Self::remove_listener(&self.listeners.connection_listeners, listener);
    }

    pub fn add_message_listener(&self, listener: Arc<dyn MessageListener>) {
        self.listeners
            .message_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_message_listener(&self, listener: &Arc<dyn MessageListener>) {
        Self::remove_listener(&self.listeners.message_listeners, listener);
    }

    pub fn add_channel_listener(&self, listener: Arc<dyn ChannelListener>) {
        self.listeners
            .channel_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_channel_listener(&self, listener: &Arc<dyn ChannelListener>) {
        Self::remove_listener(&self.listeners.channel_listeners, listener);
    }

    pub fn add_contact_listener(&self, listener: Arc<dyn ContactListener>) {
        self.listeners
            .contact_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_contact_listener(&self, listener: &Arc<dyn ContactListener>) {
        Self::remove_listener(&self.listeners.contact_listeners, listener);
    }

    pub fn add_session_listener(&self, listener: Arc<dyn SessionListener>) {
        self.listeners
            .session_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_session_listener(&self, listener: &Arc<dyn SessionListener>) {
        Self::remove_listener(&self.listeners.session_listeners, listener);
    }

    pub fn add_friend_request_listener(&self, listener: Arc<dyn FriendRequestListener>) {
        self.listeners
            .friend_request_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_friend_request_listener(&self, listener: &Arc<dyn FriendRequestListener>) {
        Self::remove_listener(&self.listeners.friend_request_listeners, listener);
    }

    pub fn remove_all_listeners(&self) {
        self.listeners.connection_listeners.write().unwrap().clear();
        self.listeners.message_listeners.write().unwrap().clear();
        self.listeners.channel_listeners.write().unwrap().clear();
        self.listeners.contact_listeners.write().unwrap().clear();
        self.listeners.session_listeners.write().unwrap().clear();
        self.listeners
            .friend_request_listeners
            .write()
            .unwrap()
            .clear();
    }

    pub fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(ComposedMessageBuilder::new(recipient))
    }
}

impl MessagingClient for Client {
    fn user_id(&self) -> &Id {
        self.user_id()
    }

    fn device_id(&self) -> &Id {
        self.device_id()
    }

    fn service_peer_id(&self) -> &Id {
        self.service_peer_id()
    }

    fn service_endpoint(&self) -> Option<&str> {
        self.service_endpoint()
    }

    fn data_dir(&self) -> &Path {
        self.data_dir()
    }

    fn start(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.start().await })
    }

    fn stop(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.stop().await })
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

    fn add_connection_listener(&self, listener: Arc<dyn ConnectionListener>) {
        self.add_connection_listener(listener);
    }

    fn remove_connection_listener(&self, listener: &Arc<dyn ConnectionListener>) {
        self.remove_connection_listener(listener);
    }

    fn add_message_listener(&self, listener: Arc<dyn MessageListener>) {
        self.add_message_listener(listener);
    }

    fn remove_message_listener(&self, listener: &Arc<dyn MessageListener>) {
        self.remove_message_listener(listener);
    }

    fn add_channel_listener(&self, listener: Arc<dyn ChannelListener>) {
        self.add_channel_listener(listener);
    }

    fn remove_channel_listener(&self, listener: &Arc<dyn ChannelListener>) {
        self.remove_channel_listener(listener);
    }

    fn add_contact_listener(&self, listener: Arc<dyn ContactListener>) {
        self.add_contact_listener(listener);
    }

    fn remove_contact_listener(&self, listener: &Arc<dyn ContactListener>) {
        self.remove_contact_listener(listener);
    }

    fn add_session_listener(&self, listener: Arc<dyn SessionListener>) {
        self.add_session_listener(listener);
    }

    fn remove_session_listener(&self, listener: &Arc<dyn SessionListener>) {
        self.remove_session_listener(listener);
    }

    fn add_friend_request_listener(&self, listener: Arc<dyn FriendRequestListener>) {
        self.add_friend_request_listener(listener);
    }

    fn remove_friend_request_listener(&self, listener: &Arc<dyn FriendRequestListener>) {
        self.remove_friend_request_listener(listener);
    }

    fn remove_all_listeners(&self) {
        self.remove_all_listeners();
    }

    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        self.message(recipient)
    }

    fn get_conversation(&self, _id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Conversation>>>> {
        self.unsupported("get_conversation")
    }

    fn get_conversations(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Conversation>>>> {
        self.unsupported("get_conversations")
    }

    fn remove_conversation(&self, _id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_conversation")
    }

    fn remove_conversations(&self, _ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_conversations")
    }

    fn get_messages(
        &self,
        _conversation_id: &Id,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>> {
        self.unsupported("get_messages")
    }

    fn get_messages_in_range(
        &self,
        _conversation_id: &Id,
        _begin: i64,
        _end: i64,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>> {
        self.unsupported("get_messages_in_range")
    }

    fn remove_message(&self, _message_id: i64) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_message")
    }

    fn remove_messages_by_ids(&self, _message_ids: &[i64]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_messages_by_ids")
    }

    fn remove_messages_in_conversation(&self, _conversation_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_messages_in_conversation")
    }

    fn get_sessions(&self) -> BoxFuture<'_, Result<Vec<SessionInfo>>> {
        self.unsupported("get_sessions")
    }

    fn revoke_session(&self, _device_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("revoke_session")
    }

    fn friend_request(&self, _user_id: Id, _hello: Option<String>) -> BoxFuture<'_, Result<()>> {
        self.unsupported("friend_request")
    }

    fn accept_friend_request(&self, _user_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("accept_friend_request")
    }

    fn get_friend_request(
        &self,
        _user_id: &Id,
    ) -> BoxFuture<'_, Result<Option<Box<dyn FriendRequest>>>> {
        self.unsupported("get_friend_request")
    }

    fn get_friend_requests(&self) -> BoxFuture<'_, Result<Vec<Box<dyn FriendRequest>>>> {
        self.unsupported("get_friend_requests")
    }

    fn remove_friend_request(&self, _user_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_friend_request")
    }

    fn remove_friend_requests(&self, _user_ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_friend_requests")
    }

    fn clear_friend_requests(&self) -> BoxFuture<'_, Result<()>> {
        self.unsupported("clear_friend_requests")
    }

    fn add_friend(
        &self,
        _user_id: Id,
        _session_key: Vec<u8>,
        _remark: Option<String>,
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("add_friend")
    }

    fn create_channel(
        &self,
        _permission: Permission,
        _name: String,
        _notice: Option<String>,
        _announcement: Option<String>,
    ) -> BoxFuture<'_, Result<Box<dyn Channel>>> {
        self.unsupported("create_channel")
    }

    fn remove_channel(&self, _channel_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_channel")
    }

    fn join_channel(&self, _ticket: InviteTicket) -> BoxFuture<'_, Result<Box<dyn Channel>>> {
        self.unsupported("join_channel")
    }

    fn leave_channel(&self, _channel_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("leave_channel")
    }

    fn create_invite_ticket(
        &self,
        _channel_id: &Id,
        _invitee: Option<Id>,
    ) -> BoxFuture<'_, Result<InviteTicket>> {
        self.unsupported("create_invite_ticket")
    }

    fn transfer_channel_ownership(
        &self,
        _channel_id: &Id,
        _new_owner: Id,
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("transfer_channel_ownership")
    }

    fn rotate_channel_session_key(&self, _channel_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("rotate_channel_session_key")
    }

    fn update_channel_info(&self, _channel: &dyn Channel) -> BoxFuture<'_, Result<()>> {
        self.unsupported("update_channel_info")
    }

    fn set_channel_members_role(
        &self,
        _channel_id: &Id,
        _members: &[Id],
        _role: Role,
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("set_channel_members_role")
    }

    fn ban_channel_members(&self, _channel_id: &Id, _members: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("ban_channel_members")
    }

    fn unban_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("unban_channel_members")
    }

    fn remove_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_channel_members")
    }

    fn get_contact(&self, _id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Contact>>>> {
        self.unsupported("get_contact")
    }

    fn get_contacts(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Contact>>>> {
        self.unsupported("get_contacts")
    }

    fn update_contact(&self, _contact: &dyn Contact) -> BoxFuture<'_, Result<()>> {
        self.unsupported("update_contact")
    }

    fn remove_contact(&self, _id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_contact")
    }

    fn remove_contacts(&self, _ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_contacts")
    }

    fn clear_contacts(&self) -> BoxFuture<'_, Result<()>> {
        self.unsupported("clear_contacts")
    }
}

#[allow(dead_code)]
struct ComposedMessageBuilder {
    recipient: Option<Id>,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

impl ComposedMessageBuilder {
    fn new(recipient: Option<Id>) -> Self {
        Self {
            recipient,
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
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
        self
    }

    fn binary_body(mut self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilder> {
        self.body = Some(data);
        self
    }

    fn header(mut self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilder> {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }
}
