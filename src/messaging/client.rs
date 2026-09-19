use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, SubscribeFilter, Transport};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::Duration;

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

const USER_INBOX: &str = "u/i";
const USER_OUTBOX: &str = "u/o";
const DEVICE_INBOX: &str = "d/i";
const MAX_MESSAGE_SIZE: usize = 256 * 1024;

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
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>>;

    /// Retrieve messages from a time range `[begin, end)` (milliseconds).
    fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
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
    fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> BoxFuture<'_, Result<Option<Box<dyn FriendRequest>>>>;

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
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> BoxFuture<'_, Result<()>>;

    // -----------------------------------------------------------------
    // Channels
    // -----------------------------------------------------------------

    /// Create a new channel.
    fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
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
        invitee: Option<Id>,
    ) -> BoxFuture<'_, Result<InviteTicket>>;

    /// Transfer channel ownership to another user.
    fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner: Id,
    ) -> BoxFuture<'_, Result<()>>;

    /// Rotate the channel session key, optionally supplying a pre-generated keypair.
    fn rotate_channel_session_key(&self, channel_id: &Id) -> BoxFuture<'_, Result<()>>;

    /// Update channel metadata.
    fn update_channel_info(&self, channel: &dyn Channel) -> BoxFuture<'_, Result<()>>;

    /// Update the roles of a set of channel members.
    fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
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

struct Shared {
    running: AtomicBool,
    connected: AtomicBool,
    ready: AtomicBool,
    mqtt: Mutex<Option<AsyncClient>>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    connection_listeners: RwLock<Vec<Arc<dyn ConnectionListener>>>,
    message_listeners: RwLock<Vec<Arc<dyn MessageListener>>>,
    channel_listeners: RwLock<Vec<Arc<dyn ChannelListener>>>,
    contact_listeners: RwLock<Vec<Arc<dyn ContactListener>>>,
    session_listeners: RwLock<Vec<Arc<dyn SessionListener>>>,
    friend_request_listeners: RwLock<Vec<Arc<dyn FriendRequestListener>>>,
}

/// The Boson Messaging Client implementation.
pub struct Client {
    options: Options,
    user_id: Id,
    device_id: Id,
    shared: Arc<Shared>,
}

pub type PhotonMessagingClient = Client;

impl Client {
    pub fn new(options: Options) -> Self {
        let shared = Shared {
            running: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            mqtt: Mutex::new(None),
            worker: Mutex::new(None),
            connection_listeners: RwLock::new(Vec::new()),
            message_listeners: RwLock::new(Vec::new()),
            channel_listeners: RwLock::new(Vec::new()),
            contact_listeners: RwLock::new(Vec::new()),
            session_listeners: RwLock::new(Vec::new()),
            friend_request_listeners: RwLock::new(Vec::new()),
        };

        let user_id = options
            .user_id
            .or_else(|| options.user_key.as_ref().map(|k| Id::from(k.public_key())))
            .unwrap_or_else(Id::random);
        let device_id = options
            .device_id
            .or_else(|| {
                options
                    .device_key
                    .as_ref()
                    .map(|k| Id::from(k.public_key()))
            })
            .unwrap_or_else(Id::random);

        Self {
            user_id,
            device_id,
            options,
            shared: Arc::new(shared),
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

    fn password(&self) -> Result<String> {
        let nonce = crate::cryptobox::Nonce::random();
        let device_key = self
            .options
            .device_key
            .as_ref()
            .ok_or_else(|| Error::State("device_key is required".into()))?;
        let user_key = self.options.user_key.as_ref().unwrap_or(device_key);

        let usign = user_key
            .private_key()
            .sign_into(nonce.as_bytes())
            .map_err(|error| Error::Auth(error.to_string()))?;
        let dsign = device_key
            .private_key()
            .sign_into(nonce.as_bytes())
            .map_err(|error| Error::Auth(error.to_string()))?;

        let mut password = Vec::with_capacity(nonce.size() + usign.len() + dsign.len());
        password.extend_from_slice(nonce.as_bytes());
        password.extend_from_slice(&usign);
        password.extend_from_slice(&dsign);

        Ok(bs58::encode(password).into_string())
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        let listeners = self.shared.connection_listeners.read().unwrap().clone();
        for listener in listeners {
            callback(listener.as_ref());
        }
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
        &self.user_id
    }

    pub fn device_id(&self) -> &Id {
        &self.device_id
    }

    pub fn service_peer_id(&self) -> &Id {
        &self.options.peerid
    }

    pub fn service_endpoint(&self) -> Option<&str> {
        self.options.endpoint.as_ref().map(url::Url::as_str)
    }

    pub fn data_dir(&self) -> &Path {
        self.options.data_dir.as_path()
    }

    pub fn start(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            if self
                .shared
                .running
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Ok(());
            }

            let result = async {
                let endpoint = self.options.endpoint.as_ref().ok_or_else(|| {
                    Error::State(
                        "service.endpoint is required: DHT service discovery is not yet wired \
                         into the updated Rust messaging Options"
                            .into(),
                    )
                })?;
                tokio::fs::create_dir_all(&self.options.data_dir).await?;

                let host = endpoint
                    .host_str()
                    .ok_or_else(|| Error::Argument("service endpoint has no hostname".into()))?;
                let port = endpoint
                    .port()
                    .ok_or_else(|| Error::Argument("service endpoint has no port".into()))?;

                self.notify_connection(|listener| listener.on_connecting());

                let client_id =
                    bs58::encode(md5::compute(self.device_id.as_bytes()).0).into_string();
                let mut options = MqttOptions::new(client_id, host.to_string(), port);
                options.set_credentials(self.user_id.to_string(), self.password()?);
                options.set_keep_alive(Duration::from_secs(60));
                options.set_clean_session(false);
                options.set_max_packet_size(MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE);
                if endpoint.scheme() == "mqtts" || endpoint.scheme() == "ssl" {
                    options.set_transport(Transport::tls_with_default_config());
                }

                let (mqtt, mut eventloop) = AsyncClient::new(options, 32);
                let userid = self.user_id.to_string();
                mqtt.subscribe_many([
                    SubscribeFilter::new(format!("inbox/{userid}"), QoS::AtLeastOnce),
                    SubscribeFilter::new(format!("outbox/{userid}"), QoS::AtLeastOnce),
                    SubscribeFilter::new("broadcast".to_string(), QoS::AtLeastOnce),
                    SubscribeFilter::new(USER_INBOX.to_string(), QoS::AtLeastOnce),
                    SubscribeFilter::new(USER_OUTBOX.to_string(), QoS::AtLeastOnce),
                    SubscribeFilter::new(DEVICE_INBOX.to_string(), QoS::AtLeastOnce),
                ])
                .await
                .map_err(|error| Error::Io(std::io::Error::other(error)))?;

                *self.shared.mqtt.lock().unwrap() = Some(mqtt);
                let shared = self.shared.clone();
                let worker = tokio::spawn(async move {
                    while shared.running.load(Ordering::Acquire) {
                        match eventloop.poll().await {
                            Ok(Event::Incoming(Incoming::ConnAck(connack))) => {
                                if connack.code == rumqttc::ConnectReturnCode::Success {
                                    if !shared.connected.swap(true, Ordering::AcqRel) {
                                        let listeners =
                                            shared.connection_listeners.read().unwrap().clone();
                                        for listener in listeners {
                                            listener.on_connected();
                                        }
                                    }
                                } else {
                                    log::warn!(
                                        "Messaging MQTT ConnAck error code: {:?}",
                                        connack.code
                                    );
                                }
                            }
                            Ok(Event::Incoming(Incoming::SubAck(_))) => {
                                if !shared.connected.swap(true, Ordering::AcqRel) {
                                    let listeners =
                                        shared.connection_listeners.read().unwrap().clone();
                                    for listener in listeners {
                                        listener.on_connected();
                                    }
                                }
                                if !shared.ready.swap(true, Ordering::AcqRel) {
                                    let listeners =
                                        shared.connection_listeners.read().unwrap().clone();
                                    for listener in listeners {
                                        listener.on_ready();
                                    }
                                }
                            }
                            Ok(Event::Incoming(Incoming::Publish(_))) => {}
                            Ok(_) => {}
                            Err(error) => {
                                if shared.connected.swap(false, Ordering::AcqRel) {
                                    shared.ready.store(false, Ordering::Release);
                                    let listeners =
                                        shared.connection_listeners.read().unwrap().clone();
                                    for listener in listeners {
                                        listener.on_disconnected();
                                    }
                                }
                                if shared.running.load(Ordering::Acquire) {
                                    log::warn!("Messaging MQTT connection error: {error}");
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                }
                            }
                        }
                    }
                    if shared.connected.swap(false, Ordering::AcqRel) {
                        let listeners = shared.connection_listeners.read().unwrap().clone();
                        for listener in listeners {
                            listener.on_disconnected();
                        }
                    }
                    shared.ready.store(false, Ordering::Release);
                });
                *self.shared.worker.lock().unwrap() = Some(worker);
                Ok(())
            }
            .await;

            if result.is_err() {
                self.shared.running.store(false, Ordering::Release);
            }
            result
        })
    }

    pub fn stop(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            if !self.shared.running.swap(false, Ordering::AcqRel) {
                return Ok(());
            }

            let mqtt = self.shared.mqtt.lock().unwrap().take();
            if let Some(mqtt) = mqtt {
                let _ = mqtt.disconnect().await;
            }
            let worker = self.shared.worker.lock().unwrap().take();
            if let Some(mut worker) = worker {
                if tokio::time::timeout(Duration::from_secs(2), &mut worker)
                    .await
                    .is_err()
                {
                    worker.abort();
                    let _ = worker.await;
                }
            }
            self.shared.connected.store(false, Ordering::Release);
            self.shared.ready.store(false, Ordering::Release);
            Ok(())
        })
    }

    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }

    pub fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        self.shared.ready.load(Ordering::Acquire)
    }

    pub fn add_connection_listener(&self, listener: Arc<dyn ConnectionListener>) {
        self.shared
            .connection_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_connection_listener(&self, listener: &Arc<dyn ConnectionListener>) {
        Self::remove_listener(&self.shared.connection_listeners, listener);
    }

    pub fn add_message_listener(&self, listener: Arc<dyn MessageListener>) {
        self.shared
            .message_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_message_listener(&self, listener: &Arc<dyn MessageListener>) {
        Self::remove_listener(&self.shared.message_listeners, listener);
    }

    pub fn add_channel_listener(&self, listener: Arc<dyn ChannelListener>) {
        self.shared
            .channel_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_channel_listener(&self, listener: &Arc<dyn ChannelListener>) {
        Self::remove_listener(&self.shared.channel_listeners, listener);
    }

    pub fn add_contact_listener(&self, listener: Arc<dyn ContactListener>) {
        self.shared
            .contact_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_contact_listener(&self, listener: &Arc<dyn ContactListener>) {
        Self::remove_listener(&self.shared.contact_listeners, listener);
    }

    pub fn add_session_listener(&self, listener: Arc<dyn SessionListener>) {
        self.shared
            .session_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_session_listener(&self, listener: &Arc<dyn SessionListener>) {
        Self::remove_listener(&self.shared.session_listeners, listener);
    }

    pub fn add_friend_request_listener(&self, listener: Arc<dyn FriendRequestListener>) {
        self.shared
            .friend_request_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    pub fn remove_friend_request_listener(&self, listener: &Arc<dyn FriendRequestListener>) {
        Self::remove_listener(&self.shared.friend_request_listeners, listener);
    }

    pub fn remove_all_listeners(&self) {
        self.shared.connection_listeners.write().unwrap().clear();
        self.shared.message_listeners.write().unwrap().clear();
        self.shared.channel_listeners.write().unwrap().clear();
        self.shared.contact_listeners.write().unwrap().clear();
        self.shared.session_listeners.write().unwrap().clear();
        self.shared
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
        self.start()
    }

    fn stop(&self) -> BoxFuture<'_, Result<()>> {
        self.stop()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature;

    #[tokio::test]
    async fn start_without_endpoint_fails_and_rolls_back_running_state() {
        let options = Options::new(Id::random()).with_generated_device_key();
        let client = Client::new(options);

        let error = client.start().await.unwrap_err();
        assert!(error.to_string().contains("service.endpoint is required"));
        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());
    }

    #[test]
    fn identities_are_derived_from_options_keys() {
        let user_key = signature::KeyPair::random();
        let device_key = signature::KeyPair::random();
        let expected_user = Id::from(user_key.public_key());
        let expected_device = Id::from(device_key.public_key());
        let options = Options::new(Id::random())
            .with_user_keypair(user_key)
            .with_device_keypair(device_key);
        let client = Client::new(options);

        assert_eq!(client.user_id(), &expected_user);
        assert_eq!(client.device_id(), &expected_device);
    }
}
