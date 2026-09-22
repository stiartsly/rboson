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

use crate::messaging::{
    channel::{Channel, Permission, Role},
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::{Contact, ContactEditor, ContactType},
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

/// Concrete implementation of a [`Contact`].
#[derive(Debug, Clone)]
pub struct PhotonContact {
    pub id: Id,
    pub contact_type: ContactType,
    pub name: Option<String>,
    pub remark: Option<String>,
    pub tags: Option<String>,
    pub muted: bool,
    pub blocked: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i32,
}

impl Contact for PhotonContact {
    fn id(&self) -> &Id {
        &self.id
    }

    fn contact_type(&self) -> ContactType {
        self.contact_type
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn remark(&self) -> Option<&str> {
        self.remark.as_deref()
    }

    fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    fn is_muted(&self) -> bool {
        self.muted
    }

    fn is_blocked(&self) -> bool {
        self.blocked
    }

    fn created_at(&self) -> i64 {
        self.created_at
    }

    fn updated_at(&self) -> i64 {
        self.updated_at
    }

    fn revision(&self) -> i32 {
        self.revision
    }

    fn edit(&self) -> Box<dyn ContactEditor> {
        Box::new(PhotonContactEditor {
            contact: self.clone(),
        })
    }
}

/// Builder for updating a [`PhotonContact`].
pub struct PhotonContactEditor {
    contact: PhotonContact,
}

impl ContactEditor for PhotonContactEditor {
    fn remark(mut self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditor> {
        self.contact.remark = remark;
        self
    }

    fn tags(mut self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditor> {
        self.contact.tags = tags;
        self
    }

    fn muted(mut self: Box<Self>, muted: bool) -> Box<dyn ContactEditor> {
        self.contact.muted = muted;
        self
    }

    fn blocked(mut self: Box<Self>, blocked: bool) -> Box<dyn ContactEditor> {
        self.contact.blocked = blocked;
        self
    }

    fn build(mut self: Box<Self>) -> Box<dyn Contact> {
        self.contact.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.contact.revision += 1;
        Box::new(self.contact)
    }
}

struct ClientState {
    friend_requests: HashMap<Id, PhotonFriendRequest>,
    contacts: HashMap<Id, PhotonContact>,
}

/// The Boson Messaging Client implementation.
pub struct Client {
    options: Options,
    listeners: Arc<SharedListeners>,
    verticle: Mutex<Option<VerticleClient>>,
    state: Arc<RwLock<ClientState>>,
}

pub type PhotonMessagingClient = Client;

impl Client {
    pub fn new(options: Options) -> Self {
        let state = ClientState {
            friend_requests: HashMap::new(),
            contacts: HashMap::new(),
        };
        Self {
            options,
            listeners: Arc::new(SharedListeners::new()),
            verticle: Mutex::new(None),
            state: Arc::new(RwLock::new(state)),
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

    pub fn director_node_id(&self) -> Option<&Id> {
        self.options.director_node_id()
    }

    pub fn director_endpoint(&self) -> Option<&str> {
        self.options.director_endpoint().map(url::Url::as_str)
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

    pub async fn friend_request(&self, user_id: Id, hello: Option<String>) -> Result<()> {
        <Self as MessagingClient>::friend_request(self, user_id, hello).await
    }

    pub async fn accept_friend_request(&self, user_id: &Id) -> Result<()> {
        <Self as MessagingClient>::accept_friend_request(self, user_id).await
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
        user_id: Id,
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

    fn director_node_id(&self) -> Option<&Id> {
        self.director_node_id()
    }

    fn director_endpoint(&self) -> Option<&str> {
        self.director_endpoint()
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

    fn connection_status(&self) -> &str {
        self.connection_status()
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

    fn friend_request(&self, user_id: Id, hello: Option<String>) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            if user_id == *self.user_id() {
                return Err(Error::Argument("Cannot send friend request to yourself".into()));
            }

            let verticle_tx = self.verticle.lock().unwrap().as_ref().map(|v| v.sender());
            if let Some(tx) = verticle_tx {
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                if tx
                    .send(verticle::VerticleEvent::FriendRequest {
                        user_id,
                        hello: hello.clone().unwrap_or_default(),
                        complete: reply_tx,
                    })
                    .is_ok()
                {
                    let _ = reply_rx.await;
                }
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

            let listeners = self
                .listeners
                .friend_request_listeners
                .read()
                .unwrap()
                .clone();
            for listener in listeners {
                listener.on_friend_request(&user_id, hello.as_deref());
            }
            Ok(())
        })
    }

    fn accept_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<()>> {
        let user_id = *user_id;
        Box::pin(async move {
            {
                let state = self.state.read().unwrap();
                let req = state.friend_requests.get(&user_id).ok_or_else(|| {
                    Error::Argument(format!("No friend request found for {user_id}"))
                })?;
                if req.initiator_id == *self.user_id() {
                    return Err(Error::State(
                        "Cannot accept your own friend request".into(),
                    ));
                }
                if req.accepted {
                    return Err(Error::State(
                        "Friend request has already been accepted".into(),
                    ));
                }
                if req.expired {
                    return Err(Error::State("Friend request has expired".into()));
                }
            }

            let verticle_tx = self.verticle.lock().unwrap().as_ref().map(|v| v.sender());
            if let Some(tx) = verticle_tx {
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                if tx
                    .send(verticle::VerticleEvent::FriendAccept {
                        user_id,
                        complete: reply_tx,
                    })
                    .is_ok()
                {
                    let _ = reply_rx.await;
                }
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

            let req_listeners = self
                .listeners
                .friend_request_listeners
                .read()
                .unwrap()
                .clone();
            for listener in req_listeners {
                listener.on_friend_request_accepted(&user_id);
            }

            let contact_listeners = self
                .listeners
                .contact_listeners
                .read()
                .unwrap()
                .clone();
            for listener in contact_listeners {
                listener.on_contact_added(&contact);
            }
            Ok(())
        })
    }

    fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> BoxFuture<'_, Result<Option<Box<dyn FriendRequest>>>> {
        let user_id = *user_id;
        Box::pin(async move {
            let state = self.state.read().unwrap();
            Ok(state
                .friend_requests
                .get(&user_id)
                .cloned()
                .map(|r| Box::new(r) as Box<dyn FriendRequest>))
        })
    }

    fn get_friend_requests(&self) -> BoxFuture<'_, Result<Vec<Box<dyn FriendRequest>>>> {
        Box::pin(async move {
            let state = self.state.read().unwrap();
            Ok(state
                .friend_requests
                .values()
                .cloned()
                .map(|r| Box::new(r) as Box<dyn FriendRequest>)
                .collect())
        })
    }

    fn remove_friend_request(&self, user_id: &Id) -> BoxFuture<'_, Result<()>> {
        let user_id = *user_id;
        Box::pin(async move {
            self.state.write().unwrap().friend_requests.remove(&user_id);
            Ok(())
        })
    }

    fn remove_friend_requests(&self, user_ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        let user_ids = user_ids.to_vec();
        Box::pin(async move {
            let mut state = self.state.write().unwrap();
            for id in &user_ids {
                state.friend_requests.remove(id);
            }
            Ok(())
        })
    }

    fn clear_friend_requests(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.state.write().unwrap().friend_requests.clear();
            Ok(())
        })
    }

    fn add_friend(
        &self,
        user_id: Id,
        _session_key: Vec<u8>,
        remark: Option<String>,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let now = SystemTime::now();
            let now_ms = now
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let contact = PhotonContact {
                id: user_id,
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
            self.state.write().unwrap().contacts.insert(user_id, contact.clone());

            let listeners = self
                .listeners
                .contact_listeners
                .read()
                .unwrap()
                .clone();
            for listener in listeners {
                listener.on_contact_added(&contact);
            }
            Ok(())
        })
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

    fn get_contact(&self, id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Contact>>>> {
        let id = *id;
        Box::pin(async move {
            let state = self.state.read().unwrap();
            Ok(state
                .contacts
                .get(&id)
                .cloned()
                .map(|c| Box::new(c) as Box<dyn Contact>))
        })
    }

    fn get_contacts(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Contact>>>> {
        Box::pin(async move {
            let state = self.state.read().unwrap();
            Ok(state
                .contacts
                .values()
                .cloned()
                .map(|c| Box::new(c) as Box<dyn Contact>)
                .collect())
        })
    }

    fn update_contact(&self, contact: &dyn Contact) -> BoxFuture<'_, Result<()>> {
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
        Box::pin(async move {
            self.state
                .write()
                .unwrap()
                .contacts
                .insert(updated.id, updated.clone());
            let listeners = self.listeners.contact_listeners.read().unwrap().clone();
            for listener in listeners {
                let boxed: Box<dyn Contact> = Box::new(updated.clone());
                listener.on_contacts_updated(&[boxed]);
            }
            Ok(())
        })
    }

    fn remove_contact(&self, id: &Id) -> BoxFuture<'_, Result<()>> {
        let id = *id;
        Box::pin(async move {
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
            let listeners = self.listeners.contact_listeners.read().unwrap().clone();
            for listener in listeners {
                listener.on_contacts_removed(&[id]);
            }
            Ok(())
        })
    }

    fn remove_contacts(&self, ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        let ids = ids.to_vec();
        Box::pin(async move {
            let mut state = self.state.write().unwrap();
            for id in &ids {
                state.contacts.remove(id);
            }
            drop(state);
            let listeners = self.listeners.contact_listeners.read().unwrap().clone();
            for listener in listeners {
                listener.on_contacts_removed(&ids);
            }
            Ok(())
        })
    }

    fn clear_contacts(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.state.write().unwrap().contacts.clear();
            let listeners = self.listeners.contact_listeners.read().unwrap().clone();
            for listener in listeners {
                listener.on_contacts_cleared();
            }
            Ok(())
        })
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
