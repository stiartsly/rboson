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
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use futures::{
    stream::FuturesUnordered,
    FutureExt,
};
use log::{debug, error, info, warn};
use serde::Deserialize;
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use tokio::sync;

use boson_api::{
    Result,
    errors::{ArgumentError, StateError},
    signature,
    CryptoIdentity,
    Id,
    Identity,
};

use crate::{
    channel::{Channel, Permission, Role},
    contact::{Contact, ContactType},
    content_disposition::ContentDisposition,
    content_type,
    errors::{
        AuthenticationError,
        EncodingError,
        ProtocolError,
    },
    message::MessageType,
    options::Options,
    session_info::SessionInfo,
    ChannelListener,
    ChannelMember,
    ConnectionListener,
    ContactListener,
    FriendRequest,
    FriendRequestListener,
    InviteTicket,
    Message,
    MessageListener,
    SessionListener,
};

use crate::interfaces::{
    ChannelMemberOps,
    ConversationOps,
    MessageBuilderOps,
    MessageOps,
};

use crate::internal::{
    ChannelInfo,
    ChannelSessionKeyRotation,
    ContactData,
    ContactMutation,
    ContactSync,
    ContentFormat,
    ChannelInfoUpdate,
    JoinChannel,
    Handshake,
    HandshakeType,
    PhotonContent,
    MutationData,
    NewChannelInfo,
    Notification,
    NotificationBody,
    NotificationEvent,
    OpaqueContact,
    PhotonChannel,
    PhotonContact,
    PhotonMessage,
    PhotonConversation,
    SessionContext,
    SyncType,
    Topic,
    MessagingRepository,
};

use crate::rpc::{
    call::Calls,
    error::RPCError,
    request::RPCRequest,
    response::{
        Params as ResponseParams,
        RPCResponseMessage,
    },
};

use mqtt::{
    decode_cbor,
    decode_cbor_bytes,
    to_cbor,
    to_cbor_bytes,
    ClientHandle,
    MqttClient,
};
use super::verticle::VerticleOptions;

const MESSAGE_VERSION: u8 = 2;
const HANDSHAKE_MESSAGE: u8 = 0;

const RPC_TIMEOUT: Duration = Duration::from_secs(15);

// Field names of the CBOR documents exchanged with the service. They are short
// because every message carries them, so they get names here instead of being
// spelled out at each call site.
const FIELD_SESSION_KEY: &str = "sk";
const FIELD_OWNER: &str = "o";
const FIELD_PERMISSION: &str = "p";
const FIELD_NOTICE: &str = "nt";
const FIELD_ANNOUNCED: &str = "a";

#[derive(Deserialize)]
struct ChannelContactInfo {
    #[serde(rename = "o")]
    owner: Id,
    #[serde(rename = "p")]
    permission: i32,
    #[serde(rename = "nt", default)]
    notice: Option<String>,
    #[serde(rename = "a", default)]
    announced: bool,
}

/// A contact as decoded from the wire, together with the session material that
/// travels alongside it.
///
/// Friend contacts carry a session key, channel contacts carry a session key
/// *and* the channel metadata, so the two optional fields are independent.
struct DecodedContact {
    contact: Contact,
    session: Option<CryptoIdentity>,
    channel: Option<Channel>,
}

/// The contact book a sync is applied to.
///
/// A sync mutates a copy and is only committed once every revision check has
/// passed, so a malformed delta can never leave the session half updated.
struct ContactState {
    contacts: HashMap<Id, Contact>,
    sessions: HashMap<Id, SessionContext>,
    channels: HashMap<Id, Channel>,
    revision: i32,
}

pub(crate) struct RpcServer {
    options: Arc<Options>,
    transport: Rc<MqttClient>,
    running: Cell<bool>,
    connected: Cell<bool>,
    ready: Cell<bool>,
    reconnect_at: Cell<Option<tokio::time::Instant>>,
    sync_running: Cell<bool>,
    stopping: Cell<bool>,

    lifecycle: sync::Mutex<()>,
    requests: sync::Mutex<()>,

    origin_clock: Arc<std::sync::Mutex<i64>>,
    pending_rpc: Calls,
    contacts_revision: Cell<i32>,
    contact_sync_needed: Cell<bool>,

    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,

    friend_sessions: Arc<Mutex<HashMap<Id, SessionContext>>>,
    blocked_contacts: Arc<Mutex<HashSet<Id>>>,

    friend_requests: RefCell<HashMap<Id, FriendRequest>>,


    repository: Rc<RefCell<MessagingRepository>>,

    channels_cache: RefCell<HashMap<Id, Channel>>,
    contacts_cache: RefCell<HashMap<Id, Contact>>,
    conversations_cache: Arc<Mutex<HashMap<Id, PhotonConversation>>>,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn FriendRequestListener>,
}

// ---------------------------------------------------------------------------
// CBOR document helpers
// ---------------------------------------------------------------------------

/// Whether the body describes a channel change rather than a peer event.
fn is_channel_notification(body: &NotificationBody) -> bool {
    matches!(
        body,
        NotificationBody::ChannelCreate(_)
            | NotificationBody::ChannelJoin(_)
            | NotificationBody::ChannelDelete
            | NotificationBody::ChannelLeave
            | NotificationBody::ChannelSessionKeyRotate(_)
            | NotificationBody::ChannelMemberJoin(_)
            | NotificationBody::ChannelMemberLeave(_)
    )
}

/// Whether a notification is the echo of an operation this device performed.
///
/// The service relays our own channel operations back to us; their local effect
/// is already applied, so replaying them would double-notify the listeners.
fn notification_echoes_back(
    notification: &Notification,
    timestamp: i64,
    user_id: &Id,
    device_id: &Id,
) -> Result<bool> {
    if notification.source() != *user_id || !is_channel_notification(notification.body()) {
        return Ok(false);
    }
    Ok(notification.id() == RpcServer::message_id(device_id, timestamp)?)
}

/// What a member-management RPC does to the local channel once the service has
/// reported which members it actually touched.
#[derive(Clone, Copy)]
enum MemberUpdate {
    /// Assign the given role.
    Role(Role),
    /// Mark as banned.
    Ban,
    /// Restore the default role.
    Unban,
    /// Drop from the channel.
    Remove,
}

impl MemberUpdate {
    /// Applies the update to `channel` and returns the members that changed.
    fn apply(self, channel: &mut Channel, touched: &[Id]) -> Vec<ChannelMember> {
        match self {
            MemberUpdate::Role(role) => Self::reassign(channel, touched, role),
            MemberUpdate::Ban => Self::reassign(channel, touched, Role::Banned),
            MemberUpdate::Unban => Self::reassign(channel, touched, Role::Member),
            MemberUpdate::Remove => channel.remove_members(touched),
        }
    }

    fn reassign(channel: &mut Channel, touched: &[Id], role: Role) -> Vec<ChannelMember> {
        channel.update_member_roles(touched, role)
    }
}

// ---------------------------------------------------------------------------
// Message pipeline
// ---------------------------------------------------------------------------
//
// Every operation that talks to the service is a short pipeline of stages, and
// each stage has exactly one owner:
//
// Outbound (client -> service)
//   1. collect    the operation builds its typed params from its arguments
//   2. encode     params -> CBOR bytes          (`to_cbor_bytes`, `RPCRequest::serialize`)
//   3. protect    body -> encrypted for the recipient, wrapped in a `Message`
//   4. envelope   message -> sealed for the service with the device key
//                 (3 and 4 are `Outbound::seal`)
//   5. publish    `MqttClient::publish`
//   6. await      `Calls::register` + `CallFuture` (RPCs only)
//   7. finish     decode the reply, then update local state and listeners
//
// Inbound (service -> client) is the mirror image
//   1. open       `MqttClient::interpret` opens the envelope into a `Message`
//   2. classify   `RpcServer::classify` checks version, sender, topic and type
//   3. unprotect  `unprotect_*` strip the inner encryption layer
//   4. decode     CBOR bytes -> typed value
//   5. apply      complete the pending call, or update local state and listeners

/// Outbound stages 3 and 4: protect a body and seal it for the service.
struct Outbound<'a> {
    user_identity: &'a CryptoIdentity,
    device_identity: &'a CryptoIdentity,
    peer_id: &'a Id,
}

impl Outbound<'_> {
    /// Returns the wire message together with its sealed MQTT payload.
    ///
    /// `inner` names the peer the body is encrypted for; `None` leaves it as
    /// is, because the service can already read what only it needs to see.
    fn seal(
        &self,
        recipient: &Id,
        message_type: u8,
        timestamp: i64,
        body: Vec<u8>,
        inner: Option<&Id>,
        what: &str,
    ) -> Result<(PhotonMessage, Vec<u8>)> {
        let body = match inner {
            Some(peer) => self
                .user_identity
                .encrypt_into(peer, &body)
                .map_err(|error| {
                    AuthenticationError::new(format!("Encrypting {what} failed: {error}"))
                })?,
            None => body,
        };
        let created_at = u64::try_from(timestamp)
            .map_err(|_| EncodingError::new("Message timestamp is negative"))?;
        let message = PhotonMessage::new(
            MESSAGE_VERSION,
            RpcServer::message_id(self.device_identity.id(), timestamp)?,
            *recipient,
            message_type,
            None,
            created_at,
            Value::Bytes(body),
        );
        let payload = mqtt::seal_envelope(
            self.device_identity,
            self.peer_id,
            &message,
            &format!("{what} envelope"),
        )?;
        Ok((message, payload))
    }
}

/// Inbound stage 3 helper: the bytes of a binary payload.
fn binary_payload(payload: Value, what: &str) -> Result<Vec<u8>> {
    match payload {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(EncodingError::new(format!("{what} payload is not binary")).into()),
    }
}

/// Inbound stage 3 helper: removes the layer `sender` encrypted for us.
fn decrypt_payload(
    identity: &CryptoIdentity,
    sender: &Id,
    ciphertext: &[u8],
    what: &str,
) -> Result<Vec<u8>> {
    identity.decrypt_into(sender, ciphertext).map_err(|error| {
        AuthenticationError::new(format!("Decrypting {what} failed: {error}")).into()
    })
}

/// What an inbound message is for, decided by [`RpcServer::classify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    RpcResponse,
    Notification,
    Handshake,
    Content,
}

// ---------------------------------------------------------------------------
// Construction and control RPCs
// ---------------------------------------------------------------------------

impl RpcServer {
    pub(crate) fn new(options: VerticleOptions) -> Result<Rc<Self>> {
        let user_key = options.options.user_key().clone();
        let user_identity = CryptoIdentity::from(user_key);

        let device_key = options.options.device_key().clone();
        let device_identity = CryptoIdentity::from(device_key);

        let transport = MqttClient::new(
            options.options.clone(),
            user_identity.clone(),
            device_identity.clone(),
        );

        let repository = MessagingRepository::new();

        Ok(Rc::new(Self {
            options: options.options,
            transport,
            running: Cell::new(false),
            connected: Cell::new(false),
            ready: Cell::new(false),
            reconnect_at: Cell::new(None),
            sync_running: Cell::new(false),
            stopping: Cell::new(false),
            lifecycle: tokio::sync::Mutex::new(()),
            requests: tokio::sync::Mutex::new(()),
            origin_clock: Arc::new(std::sync::Mutex::new(0)),
            pending_rpc: Calls::default(),
            contacts_revision: Cell::new(0),
            contact_sync_needed: Cell::new(false),
            user_identity,
            device_identity,
            friend_sessions: Arc::new(Mutex::new(HashMap::new())),
            blocked_contacts: Arc::new(Mutex::new(HashSet::new())),
            friend_requests: RefCell::new(HashMap::new()),

            repository: Rc::new(RefCell::new(repository)),

            channels_cache: RefCell::new(HashMap::new()),
            contacts_cache: RefCell::new(HashMap::new()),
            conversations_cache: Arc::new(Mutex::new(HashMap::new())),

            connection_listener: options.connection_listener.clone(),
            message_listener: options.message_listener.clone(),
            channel_listener: options.channel_listener.clone(),
            contact_listener: options.contact_listener.clone(),
            session_listener: options.session_listener.clone(),
            friend_request_listener: options.friend_request_listener.clone(),
        }))
    }

    /// Builds the cached session context for a contact or channel.
    fn persisted_session_key(&self, contact_id: &Id) -> Result<Vec<u8>> {
        let session = self
            .friend_sessions
            .lock()
            .unwrap()
            .get(contact_id)
            .cloned()
            .ok_or_else(|| StateError::new("Contact session key is missing"))?;
        self.user_identity
            .encrypt_into(self.user_id(), session.private_key())
    }

    fn persist_contact(&self, contact: &Contact, session_key: Option<&[u8]>) -> Result<()> {
        self.repository
            .borrow_mut()
            .put_contact(self.contacts_revision.get(), contact, session_key)
    }

    fn persist_channel(&self, channel: &Channel) -> Result<()> {
        let key = self.persisted_session_key(channel.id())?;
        self.repository
            .borrow_mut()
            .put_channel(self.contacts_revision.get(), channel, &key)
    }

    fn session_context(
        &self,
        contact_id: Id,
        session: CryptoIdentity,
        channel: bool,
    ) -> SessionContext {
        let user_identity = Arc::new(self.user_identity.clone());
        let session_identity = Arc::new(session);
        if channel {
            SessionContext::for_channel(contact_id, user_identity, session_identity)
        } else {
            SessionContext::for_user(contact_id, user_identity, session_identity)
        }
    }

    /// Rehydrates the in-memory contact/channel/conversation caches from the
    /// local SQLite store. Listener callbacks are not replayed on startup.
    fn restore_state(&self) -> Result<()> {
        let (revision, stored_contacts, friend_requests, conversations) = {
            let mut repository = self.repository.borrow_mut();
            (
                repository.contacts_revision()?,
                repository.stored_contacts()?,
                repository.friend_requests()?,
                repository.conversations()?,
            )
        };

        let mut contacts: HashMap<Id, Contact> = HashMap::new();
        let mut sessions = HashMap::new();
        let mut channels = HashMap::new();
        for stored in stored_contacts {
            let session = match stored.session_key.as_deref() {
                Some(key) => {
                    let key = self.user_identity.decrypt_into(self.user_id(), key)?;
                    let identity = CryptoIdentity::try_from(key.as_slice())?;
                    Some(self.session_context(
                        stored.id,
                        identity,
                        stored.contact_type == ContactType::Channel as i32,
                    ))
                }
                None => None,
            };

            if stored.contact_type == ContactType::Channel as i32 {
                let Some(session) = session else {
                    warn!("Skipping persisted channel {} without a session key", stored.id);
                    continue;
                };
                let Some(metadata) = stored.channel.as_ref() else {
                    warn!("Skipping persisted channel {} without metadata", stored.id);
                    continue;
                };
                let contact = self
                    .repository
                    .borrow()
                    .photon_contact_from_stored(&stored)?;
                let members = self
                    .repository
                    .borrow_mut()
                    .photon_channel_members(&stored.id)?;
                let channel: Channel = Box::new(PhotonChannel::new(
                    contact.clone(),
                    metadata.owner,
                    *session.session_id(),
                    Permission::try_from(metadata.permission)?,
                    metadata.notice.clone(),
                    metadata.announce,
                    members,
                ));
                sessions.insert(stored.id, session);
                contacts.insert(stored.id, Box::new(contact) as Contact);
                channels.insert(stored.id, channel);
            } else {
                let contact = self
                    .repository
                    .borrow()
                    .contact_from_stored(&stored)?;
                if let Some(session) = session {
                    sessions.insert(stored.id, session);
                }
                contacts.insert(stored.id, contact);
            }
        }

        let blocked = contacts
            .values()
            .filter(|contact| contact.is_blocked())
            .map(|contact| *contact.id())
            .collect();
        *self.contacts_cache.borrow_mut() = contacts;
        *self.friend_sessions.lock().unwrap() = sessions;
        *self.channels_cache.borrow_mut() = channels;
        *self.blocked_contacts.lock().unwrap() = blocked;
        *self.friend_requests.borrow_mut() = friend_requests
            .into_iter()
            .map(|request| (*request.user_id(), request))
            .collect();
        *self.conversations_cache.lock().unwrap() = conversations
            .into_iter()
            .map(|conversation| (*conversation.id(), conversation))
            .collect();
        self.contacts_revision.set(revision);
        Ok(())
    }

    fn origin_timestamp(clock: &std::sync::Mutex<i64>) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| StateError::new(format!("System clock error: {error}")))?;
        let now = i64::try_from(now.as_millis())
            .map_err(|_| StateError::new("System clock timestamp is out of range"))?;
        let mut previous = clock.lock().unwrap();
        let next = previous
            .checked_add(1)
            .ok_or_else(|| StateError::new("Message timestamp is exhausted"))?;
        *previous = now.max(next);
        Ok(*previous)
    }

    fn outbound(&self) -> Outbound<'_> {
        Outbound {
            user_identity: &self.user_identity,
            device_identity: &self.device_identity,
            peer_id: self.peer_id(),
        }
    }

    fn fail_pending_rpc(&self, reason: &str) {
        self.pending_rpc.fail_all(reason);
    }
}

// ---------------------------------------------------------------------------
// Lifecycle and event loop
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Reclaims expired local records; run periodically by the verticle timer.
    pub(crate) fn maintenance(&self) {
        let mut requests = self.friend_requests.borrow_mut();
        let before = requests.len();
        requests.retain(|_, request| !request.is_expired());
        let removed = before - requests.len();
        if removed > 0 {
            debug!("Reclaimed {removed} expired friend request(s)");
        }
    }

    /// Queues a contact resync if one is due and none is already running.
    pub(crate) fn schedule_contact_sync(
        self: &Rc<Self>,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        if self.is_running()
            && self.is_ready()
            && !self.sync_running.get()
            && self.contact_sync_needed.replace(false)
        {
            self.sync_running.set(true);
            let session = self.clone();
            pending.push(
                async move {
                    if let Err(error) = session.sync_contacts().await {
                        warn!("Contact resynchronization failed: {error}");
                    }
                    session.sync_running.set(false);
                }
                .boxed_local(),
            );
        }
    }

    fn close_transport(&self, reason: &str) {
        self.running.set(false);
        self.fail_pending_rpc(reason);
        self.transport.close();
        self.reconnect_at.set(None);
        self.ready.set(false);
        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
    }

    pub(crate) fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub(crate) fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub(crate) fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

    /// Connects the transport. Idempotent while already running.
    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stopping.set(false);
        if self.is_running() {
            debug!("Messaging session is already running");
            return Ok(());
        }

        if !self.repository.borrow().is_open() {
            self.repository
                .borrow_mut()
                .open(self.options.database_path())?;
        }
        self.restore_state()?;

        let session = self.clone();
        let revision = self.contacts_revision.get();
        self.transport
            .connect(revision, move || {
                session.notify_connection(|listener| listener.on_connecting());
            })
            .await?;
        self.reconnect_at.set(None);
        self.running.set(true);
        Ok(())
    }

    /// Disconnects the transport and releases the event loop.
    pub(crate) async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stopping.set(true);
        let _request = self.requests.lock().await;
        let result = self.stop_transport().await;
        self.close_transport("Messaging session stopped");
        self.transport.clear_eventloop().await;
        self.repository.borrow_mut().close();
        result
    }

    /// Whether the session is deployed and connected to the service.
    pub(crate) fn is_running(&self) -> bool {
        self.running.get()
    }

    /// Whether the initial contact sync arrived, i.e. traffic may flow.
    pub(crate) fn is_ready(&self) -> bool {
        self.ready.get()
    }
}

mod event_loop;
mod inbound;
mod outbound;
mod mqtt;

/*
#[cfg(test)]
#[path = "../unitests/test_rpc_server.rs"]
mod test_rpc_server;
*/
