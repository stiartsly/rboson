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
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::Mutex,
    sync::Arc,
    future::Future,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::sync;
use log::{debug, error, info, warn};
use futures::{stream::FuturesUnordered, FutureExt};
use serde::Deserialize;
use serde_cbor::Value;
use sha2::{Digest, Sha256};

use boson_api::{
    Id,
    Identity,
    signature,
    CryptoIdentity,
};
use boson_api::errors::{
    Result,
    ArgumentError,
    NotImplemented,
    StateError
};
use crate::{
    options::Options,
    verticle::VerticleOptions,
    channel::{Channel, Permission, Role},
    ChannelMember,
    contact::{Contact, ContactOps, ContactType},
    conversation::{ConversationOps, Conversation},
    content_disposition::{ContentDisposition, HEADER_NAME as CONTENT_DISPOSITION_HEADER},
    content_type,
    invite_ticket::InviteTicket,
    message::{Message, MessageBuilder, MessageType},
    session_info::SessionInfo,
    ChannelListener,
    ConnectionListener,
    ContactListener,
    MessageListener,
    SessionListener,
    FriendRequestListener,
    errors::{AuthenticationError, EncodingError, ProtocolError},
    FriendRequest,
};

use super::mqtt::{
    self,
    ClientHandle,
    Inbound,
    MqttClient,
    TransportEvent,
    decode_cbor,
    decode_cbor_bytes,
    to_cbor,
    to_cbor_bytes,
};
use super::internal::{
    ChannelInfo,
    ChannelSessionKeyRotation,
    ContactData,
    ContactMutation,
    ContactSync,
    ContentFormat,
    Handshake,
    HandshakeType,
    MessageContent,
    MutationData,
    NewChannelInfo,
    Notification,
    NotificationBody,
    NotificationEvent,
    OpaqueContact,
    PhotonChannel,
    PhotonChannelMember,
    PhotonMessage,
    SyncType,
    Topic,
};
use super::rpc::{call::{Calls, RPCRequest, RPCResponse}, method::Method};

const MESSAGE_VERSION: u8 = 2;
const HANDSHAKE_MESSAGE: u8 = 0;

// `MessageType` is an enum, so its discriminants cannot be used directly as
// match patterns; these mirror them for the dispatch below.
const CONTROL_MESSAGE: u8 = MessageType::ControlMessage as u8;
const STATE_MESSAGE: u8 = MessageType::StateMessage as u8;
const CONTENT_MESSAGE: u8 = MessageType::ContentMessage as u8;

const RPC_TIMEOUT: Duration = Duration::from_secs(15);

// Field names of the CBOR documents exchanged with the service. They are short
// because every message carries them, so they get names here instead of being
// spelled out at each call site.
const FIELD_VERSION: &str = "v";
const FIELD_OPERATION: &str = "op";
const FIELD_DATA: &str = "d";
const FIELD_ID: &str = "id";
const FIELD_NAME: &str = "n";
const FIELD_MEMBERS: &str = "ids";
const FIELD_ROLE: &str = "r";
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
    channel: Option<PhotonChannel>,
}

/// The contact book a sync is applied to.
///
/// A sync mutates a copy and is only committed once every revision check has
/// passed, so a malformed delta can never leave the session half updated.
struct ContactState {
    contacts: HashMap<Id, Contact>,
    sessions: HashMap<Id, CryptoIdentity>,
    channels: HashMap<Id, PhotonChannel>,
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

    friend_sessions: Arc<Mutex<HashMap<Id, CryptoIdentity>>>,
    blocked_contacts: Arc<Mutex<HashSet<Id>>>,

    friend_requests: RefCell<HashMap<Id, FriendRequest>>,
    contacts: RefCell<HashMap<Id, Contact>>,
    channels: RefCell<HashMap<Id, PhotonChannel>>,
    conversations: Arc<Mutex<HashMap<Id, Conversation>>>,

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

/// Rejects a join timestamp that cannot be turned into a `SystemTime`.
fn validate_joined_at(joined_ms: u64) -> Result<()> {
    UNIX_EPOCH
        .checked_add(Duration::from_millis(joined_ms))
        .map(|_| ())
        .ok_or_else(|| EncodingError::new("Channel member timestamp is out of range").into())
}

/// Rejects a member operation that names nobody.
fn ensure_members(members: &[Id]) -> Result<()> {
    if members.is_empty() {
        return Err(ArgumentError::new("No members specified"));
    }
    Ok(())
}

/// Encodes a member list the way the member RPCs expect it.
fn member_id_array(members: &[Id]) -> Value {
    Value::Array(
        members.iter()
            .map(|id| Value::Bytes(id.as_bytes().to_vec()))
            .collect(),
    )
}

/// Splices the channel-only fields into an encoded contact document.
///
/// `store_channel` and `update_contact` must produce byte-identical layouts, so
/// both build their document through this function.
fn insert_channel_fields(
    document: &mut BTreeMap<Value, Value>,
    channel: &PhotonChannel,
    session_key: Vec<u8>,
) {
    document.insert(Value::Text(FIELD_SESSION_KEY.into()), Value::Bytes(session_key));
    document.insert(
        Value::Text(FIELD_OWNER.into()),
        Value::Bytes(channel.owner.as_bytes().to_vec()),
    );
    document.insert(
        Value::Text(FIELD_PERMISSION.into()),
        Value::Integer((channel.permission as i32).into()),
    );
    document.insert(
        Value::Text(FIELD_NOTICE.into()),
        channel.notice.clone().map(Value::Text).unwrap_or(Value::Null),
    );
    document.insert(Value::Text(FIELD_ANNOUNCED.into()), Value::Bool(channel.announced));
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
    fn apply(self, channel: &mut PhotonChannel, touched: &[Id]) -> Vec<ChannelMember> {
        match self {
            MemberUpdate::Role(role) => Self::reassign(channel, touched, role),
            MemberUpdate::Ban => Self::reassign(channel, touched, Role::Banned),
            MemberUpdate::Unban => Self::reassign(channel, touched, Role::Member),
            MemberUpdate::Remove => {
                let removed: Vec<_> = channel.members.iter()
                    .filter(|member| touched.contains(member.id()))
                    .map(ChannelMember::from)
                    .collect();
                channel.members.retain(|member| !touched.contains(member.id()));
                removed
            }
        }
    }

    fn reassign(channel: &mut PhotonChannel, touched: &[Id], role: Role) -> Vec<ChannelMember> {
        channel.members.iter_mut()
            .filter(|member| touched.contains(member.id()))
            .map(|member| {
                *member = PhotonChannelMember::new(*member.id(), role, member.joined());
                ChannelMember::from(&*member)
            })
            .collect()
    }
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
            contacts: RefCell::new(HashMap::new()),
            channels: RefCell::new(HashMap::new()),
            conversations: Arc::new(Mutex::new(HashMap::new())),

            connection_listener: options.connection_listener.clone(),
            message_listener: options.message_listener.clone(),
            channel_listener: options.channel_listener.clone(),
            contact_listener: options.contact_listener.clone(),
            session_listener: options.session_listener.clone(),
            friend_request_listener: options.friend_request_listener.clone(),
        }))
    }

    fn origin_timestamp(clock: &std::sync::Mutex<i64>) -> Result<i64> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|error| StateError::new(format!("System clock error: {error}")))?;
        let now = i64::try_from(now.as_millis())
            .map_err(|_| StateError::new("System clock timestamp is out of range"))?;
        let mut previous = clock.lock().unwrap();
        let next = previous.checked_add(1)
            .ok_or_else(|| StateError::new("Message timestamp is exhausted"))?;
        *previous = now.max(next);
        Ok(*previous)
    }

    /// Sends a control RPC and waits for its reply.
    async fn rpc_call(&self, recipient: &Id, method: Method, params: Option<Value>) -> Result<Value> {
        // if self.stopping.get() || !self.connected.get() {
        //     return Err(StateError::new("Messaging client is not connected"));
        //}

        let transport = self.transport.clone();
        let timestamp = Self::origin_timestamp(&self.origin_clock)?;
        let request = RPCRequest::new(timestamp, method, params).serialize()?;
        let request = if recipient == self.peer_id() {
            request
        } else {
            self.user_identity.encrypt_into(recipient, &request)
                .map_err(|error| AuthenticationError::new(format!("Encrypting channel RPC failed: {error}")))?
        };
        let message = PhotonMessage::new(
            MESSAGE_VERSION,
            Self::message_id(self.device_id(), timestamp)?,
            *recipient,
            CONTROL_MESSAGE,
            None,
            timestamp as u64,
            Value::Bytes(request),
        );
        let payload = mqtt::seal_envelope(&self.device_identity, self.peer_id(), &message, "RPC envelope")?;

        let call = self.pending_rpc.register(timestamp, method, *recipient)?;
        tokio::time::timeout(RPC_TIMEOUT, async move {
            transport.publish(payload, "RPC request").await?;
            call.await
        }).await.map_err(|_| StateError::new(format!("RPC '{method:?}' timed out")))?
    }

    fn fail_pending_rpc(&self, reason: &str) {
        self.pending_rpc.fail_all(reason);
    }

    fn handle_rpc_response(&self, payload: Value, sender: &Id) -> Result<()> {
        let Value::Bytes(payload) = payload else {
            return Err(EncodingError::new("RPC response payload is not binary"));
        };
        self.pending_rpc.respond(RPCResponse::parse(&payload)?, sender)
    }

}

// ---------------------------------------------------------------------------
// Inbound notifications
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Decodes a service notification and dispatches it.
    ///
    /// `sender` is the peer that published it, which for channel notifications
    /// doubles as the channel id.
    fn handle_notification(&self, payload: Value, sender: &Id) -> Result<()> {
        let Value::Bytes(payload) = payload else {
            return Err(EncodingError::new("Notification payload is not binary"));
        };
        let notification: Notification = decode_cbor_bytes(&payload, "notification")?;
        let timestamp = i64::try_from(notification.timestamp())
            .map_err(|_| EncodingError::new("Notification timestamp is out of range"))?;

        // Channel changes we caused ourselves echo back through the service.
        if notification_echoes_back(&notification, timestamp, self.user_id(), self.device_id())? {
            return Ok(());
        }

        match notification.body() {
            NotificationBody::SessionNew(session) => {
                self.session_listener.on_new_session(session);
                Ok(())
            }
            // The initial contact sync is also the readiness signal: it is the
            // last thing the service sends before the session is fully usable.
            NotificationBody::ContactSync(sync) => {
                self.apply_contact_sync(sync)?;
                if self.connected.get() && !self.stopping.get() && !self.ready.replace(true) {
                    info!("Messaging session is ready");
                    self.notify_connection(|l| l.on_ready());
                }
                Ok(())
            }
            body if is_channel_notification(body) => {
                self.apply_channel_notification(&notification, sender)
            }
            body => Err(EncodingError::new(format!(
                "Unsupported home-peer notification '{}'", body.event().as_str()
            ))),
        }
    }

    /// Applies a channel notification and fires the matching listener.
    fn apply_channel_notification(&self, notification: &Notification, sender: &Id) -> Result<()> {
        let body = notification.body();
        let event = body.event();
        match body {
            NotificationBody::ChannelCreate(info) | NotificationBody::ChannelJoin(info) => {
                let (channel, session) = self.channel_from_info(info)?;
                if (event == NotificationEvent::ChannelCreate && sender != self.peer_id())
                    || (event == NotificationEvent::ChannelJoin && channel.id() != sender)
                {
                    return Err(EncodingError::new("Channel notification has an unexpected source"));
                }
                let id = *channel.id();
                let known = self.channels.borrow().contains_key(&id);
                self.friend_sessions.lock().unwrap().insert(id, session);
                self.contacts.borrow_mut().entry(id).or_insert_with(|| channel.contact.clone());
                self.channels.borrow_mut().entry(id).or_insert_with(|| channel.clone());
                if known {
                    return Ok(());
                }
                if event == NotificationEvent::ChannelCreate {
                    self.channel_listener.on_channel_created(&channel);
                } else {
                    self.channel_listener.on_joined_channel(&channel);
                }
                Ok(())
            }
            NotificationBody::ChannelDelete | NotificationBody::ChannelLeave => {
                let Some(channel) = self.channels.borrow_mut().remove(sender) else {
                    return Ok(());
                };
                self.contacts.borrow_mut().remove(sender);
                self.friend_sessions.lock().unwrap().remove(sender);
                if event == NotificationEvent::ChannelDelete {
                    self.channel_listener.on_channel_deleted(&channel);
                } else {
                    self.channel_listener.on_left_channel(&channel);
                }
                Ok(())
            }
            NotificationBody::ChannelSessionKeyRotate(encrypted_key) => {
                let mut channel = self.channel_snapshot(sender)?;
                if notification.source() != channel.owner {
                    return Err(EncodingError::new("Channel session key rotation has an unexpected source"));
                }
                let session = self.friend_sessions.lock().unwrap().get(sender).cloned()
                    .ok_or_else(|| StateError::new("Channel session key is missing"))?;
                let key = session.decrypt_into(&notification.source(), encrypted_key)
                    .map_err(|error| AuthenticationError::new(format!("Decrypting rotated channel session key failed: {error}")))?;
                let session = CryptoIdentity::try_from(key.as_slice())
                    .map_err(|error| AuthenticationError::new(format!("Invalid rotated channel session key: {error}")))?;
                channel.session_id = *session.id();
                self.friend_sessions.lock().unwrap().insert(*sender, session);
                self.channels.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener.on_channel_session_key_rotated(&channel);
                Ok(())
            }
            NotificationBody::ChannelMemberJoin(member) => {
                validate_joined_at(member.joined())?;
                let mut channel = self.channel_snapshot(sender)?;
                channel.members.retain(|existing| existing.id() != member.id());
                channel.members.push(member.clone());
                self.channels.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener.on_channel_member_joined(&channel, &ChannelMember::from(member));
                Ok(())
            }
            NotificationBody::ChannelMemberLeave(id) => {
                let mut channel = self.channel_snapshot(sender)?;
                let member = channel.members.iter().find(|member| member.id() == id).cloned();
                channel.members.retain(|member| member.id() != id);
                self.channels.borrow_mut().insert(*sender, channel.clone());
                if let Some(member) = member {
                    self.channel_listener.on_channel_member_left(&channel, &ChannelMember::from(&member));
                }
                Ok(())
            }
            body => Err(EncodingError::new(format!(
                "Unsupported channel notification '{}'", body.event().as_str()
            ))),
        }
    }

    /// Decodes a contact, keeping the session key and channel metadata that the
    /// opaque document carries for the contact kinds that have them.
    fn decode_contact(&self, opaque: &OpaqueContact) -> Result<DecodedContact> {
        let (data, plaintext) = opaque.decode(&self.user_identity)?;
        let contact = data.contact(opaque.revision())?;
        let session = data.session(&self.user_identity)?;
        let channel = match contact.contact_type() {
            ContactType::Channel => {
                let info: ChannelContactInfo = decode_cbor_bytes(&plaintext, "channel contact")?;
                let session_id = *session.as_ref()
                    .ok_or_else(|| EncodingError::new("Channel session is missing"))?.id();
                // Members are not part of the contact document, so keep whatever
                // the current channel already knows when the session matches.
                let members = self.channels.borrow().get(contact.id())
                    .filter(|channel| channel.session_id == session_id)
                    .map(|channel| channel.members.clone())
                    .unwrap_or_default();
                Some(PhotonChannel {
                    contact: contact.clone(),
                    owner: info.owner,
                    session_id,
                    permission: Permission::try_from(info.permission)?,
                    notice: info.notice,
                    announced: info.announced,
                    members,
                })
            }
            _ => None,
        };
        Ok(DecodedContact { contact, session, channel })
    }

    /// Applies a contact sync, committing only if every revision check passed.
    fn apply_contact_sync(&self, sync: &ContactSync) -> Result<()> {
        let revision = self.contacts_revision.get();
        if matches!(sync.kind(), SyncType::UpToDate) {
            if sync.revision() == revision {
                return Ok(());
            }
            self.contact_sync_needed.set(sync.revision() > revision);
            return Err(StateError::new("Contact sync revision does not match local state"));
        }

        let previous = self.contacts.borrow().clone();
        let mut state = ContactState {
            contacts: previous.clone(),
            sessions: self.friend_sessions.lock().unwrap().clone(),
            channels: self.channels.borrow().clone(),
            revision,
        };
        match sync.kind() {
            SyncType::Snapshot => self.apply_snapshot(sync, &mut state)?,
            SyncType::Delta => self.apply_delta(sync, &mut state)?,
            SyncType::UpToDate => unreachable!("handled above"),
        }
        self.commit_contact_state(previous, state)
    }

    /// Replaces the whole contact book with a snapshot.
    fn apply_snapshot(&self, sync: &ContactSync, state: &mut ContactState) -> Result<()> {
        if sync.revision() < state.revision {
            return Err(StateError::new("Invalid or outdated contact snapshot"));
        }
        state.contacts.clear();
        state.sessions.clear();
        state.channels.clear();
        for opaque in sync.contacts() {
            if opaque.revision() > sync.revision() {
                return Err(EncodingError::new("Contact revision exceeds snapshot revision"));
            }
            let DecodedContact { contact, session, channel } = self.decode_contact(&opaque)?;
            let id = *contact.id();
            if state.contacts.contains_key(&id) {
                return Err(EncodingError::new("Snapshot contains duplicate contacts"));
            }
            if let Some(session) = session {
                state.sessions.insert(id, session);
            }
            if let Some(channel) = channel {
                state.channels.insert(id, channel);
            }
            state.contacts.insert(id, contact);
        }
        state.revision = sync.revision();
        Ok(())
    }

    /// Replays an ordered list of contact mutations.
    fn apply_delta(&self, sync: &ContactSync, state: &mut ContactState) -> Result<()> {
        let original_revision = self.contacts_revision.get();
        for mutation in sync.mutations() {
            if mutation.revision() < 0 {
                return Err(EncodingError::new("Contact mutation revision is negative"));
            }
            if mutation.revision() < state.revision {
                continue;
            }
            if mutation.revision() > state.revision {
                self.contact_sync_needed.set(true);
                return Err(StateError::new("Contact delta has a revision gap"));
            }
            let next = state.revision.checked_add(1)
                .ok_or_else(|| EncodingError::new("Contact revision is exhausted"))?;

            match mutation.data() {
                Some(MutationData::Add(opaque) | MutationData::Update(opaque)) => {
                    if opaque.revision() != next {
                        return Err(EncodingError::new("Contact mutation revision is inconsistent"));
                    }
                    let DecodedContact { contact, session, channel } = self.decode_contact(opaque)?;
                    let id = *contact.id();
                    state.sessions.remove(&id);
                    if let Some(session) = session {
                        state.sessions.insert(id, session);
                    }
                    state.channels.remove(&id);
                    if let Some(channel) = channel {
                        state.channels.insert(id, channel);
                    }
                    state.contacts.insert(id, contact);
                }
                Some(MutationData::Remove(ids)) => {
                    for id in ids {
                        state.contacts.remove(&id);
                        state.sessions.remove(&id);
                        state.channels.remove(id);
                    }
                }
                None | Some(MutationData::Clear) => {
                    state.contacts.clear();
                    state.sessions.clear();
                    state.channels.clear();
                }
            }
            state.revision = next;
        }
        if state.revision < sync.revision()
            || (state.revision > sync.revision() && state.revision > original_revision)
        {
            return Err(EncodingError::new("Contact delta final revision is inconsistent"));
        }
        Ok(())
    }

    /// Publishes a validated contact book and tells the listeners what moved.
    fn commit_contact_state(&self, previous: HashMap<Id, Contact>, state: ContactState) -> Result<()> {
        let removed: Vec<_> = previous.keys()
            .filter(|id| !state.contacts.contains_key(id))
            .copied()
            .collect();
        let added: Vec<_> = state.contacts.values()
            .filter(|contact| !previous.contains_key(contact.id()))
            .cloned()
            .collect();
        let updated: Vec<Contact> = state.contacts.values()
            .filter(|contact| Self::contact_changed(previous.get(contact.id()), contact))
            .cloned()
            .collect();

        *self.blocked_contacts.lock().unwrap() = state.contacts.values()
            .filter(|contact| contact.is_blocked())
            .map(|contact| *contact.id())
            .collect();
        *self.contacts.borrow_mut() = state.contacts;
        *self.friend_sessions.lock().unwrap() = state.sessions;
        *self.channels.borrow_mut() = state.channels;
        self.contacts_revision.set(state.revision);

        if !removed.is_empty() {
            self.contact_listener.on_contacts_removed(&removed);
        }
        for contact in added {
            self.contact_listener.on_contact_added(&contact);
        }
        if !updated.is_empty() {
            self.contact_listener.on_contacts_updated(&updated);
        }
        Ok(())
    }

    /// Whether a contact differs from its stored copy in anything a listener
    /// would report. Contacts absent from `old` are additions, not updates.
    fn contact_changed(old: Option<&Contact>, new: &Contact) -> bool {
        let Some(old) = old else {
            return false;
        };
        old.revision() != new.revision()
            || old.updated_at() != new.updated_at()
            || old.is_blocked() != new.is_blocked()
            || old.is_muted() != new.is_muted()
            || old.name() != new.name()
            || old.remark() != new.remark()
            || old.tags() != new.tags()
            || old.contact_type() != new.contact_type()
    }

    /// Asks the service for everything that changed since our revision.
    async fn sync_contacts(&self) -> Result<()> {
        let result = self.rpc_call(
            self.peer_id(),
            Method::ContactSync,
            Some(Value::Integer(self.contacts_revision.get().into())),
        ).await?;
        let sync: ContactSync = decode_cbor(result, "contact sync response")?;
        self.apply_contact_sync(&sync)
    }

    /// Sends a contact mutation document and adopts the revision it reports.
    ///
    /// `minimum` is the revision the caller needs to see: plain mutations only
    /// require the current one, while `update_contact` insists on the revision
    /// it computed so that a concurrent change cannot silently win.
    async fn commit_contact_mutation(
        &self,
        params: Value,
        minimum: i32,
        outdated: &str,
    ) -> Result<i32> {
        let result = self.rpc_call(self.peer_id(), Method::ContactMutate, Some(params)).await?;
        let revision: i32 = decode_cbor(result, "contact revision")?;
        if revision < minimum || revision < self.contacts_revision.get() {
            return Err(StateError::new(outdated));
        }
        self.contacts_revision.set(revision);
        Ok(revision)
    }

    /// Encodes and sends a [`ContactMutation`].
    async fn apply_contact_mutation(
        &self,
        mutation: ContactMutation,
        minimum: i32,
        outdated: &str,
    ) -> Result<i32> {
        let params = to_cbor(&mutation, "contact mutation")?;
        self.commit_contact_mutation(params, minimum, outdated).await
    }
}

// ---------------------------------------------------------------------------
// Inbound dispatch
// ---------------------------------------------------------------------------

impl RpcServer {
    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        callback(self.connection_listener.as_ref());
    }

    /// Entry point for every inbound application payload.
    ///
    /// The transport has already opened the envelope, so only the protocol
    /// version and the message type are left to check. The device inbox only
    /// ever carries RPC replies, so anything published there is treated as one
    /// whatever type it claims to be. Everything else is routed by message type.
    async fn dispatch(&self, topic: Topic, message: PhotonMessage) -> Result<()> {
        let peer_id = self.peer_id();
        if message.version != MESSAGE_VERSION {
            return Err(ProtocolError::new(
                message.version as i32,
                "Unsupported message version",
            ));
        }
        let from = message
            .from()
            .copied()
            .ok_or_else(|| EncodingError::new("Incoming message has no sender"))?;

        if matches!(topic, Topic::DeviceInbox) {
            return self.handle_device_rpc(message, peer_id).await;
        }
        if message.message_type == STATE_MESSAGE {
            return self.handle_state_message(topic, message, peer_id).await;
        }
        if message.message_type == HANDSHAKE_MESSAGE {
            return self.handle_handshake(topic, message, from).await;
        }
        if message.message_type == CONTENT_MESSAGE {
            return self.handle_content_message(topic, message, from);
        }
        Err(EncodingError::new("Unsupported user message type"))
    }

    /// Handles a reply to a control RPC.
    ///
    /// Replies from the service are sealed with the device key only. Replies
    /// relayed on behalf of a channel peer carry a second layer that only this
    /// user can open, so they are decrypted twice.
    async fn handle_device_rpc(&self, message: PhotonMessage, peer_id: &Id) -> Result<()> {
        if message.message_type != CONTROL_MESSAGE
            || (message.recipient != *self.user_id() && message.recipient != *self.device_id())
        {
            return Err(EncodingError::new("Invalid device RPC response"));
        }
        let from = message
            .from()
            .copied()
            .ok_or_else(|| EncodingError::new("Incoming message has no sender"))?;
        if from == *peer_id {
            return self.handle_rpc_response(message.payload, &from);
        }

        let Value::Bytes(payload) = message.payload else {
            return Err(EncodingError::new("Channel RPC response is not binary"));
        };
        let payload = self.user_identity.decrypt_into(&from, &payload)
            .map_err(|error| AuthenticationError::new(format!("Decrypting channel RPC failed: {error}")))?;
        self.handle_rpc_response(Value::Bytes(payload), &from)
    }

    /// Handles a service notification.
    ///
    /// Notifications about our own account are sealed for this user; channel
    /// notifications are sealed with the channel session key instead, which is
    /// why they need a lookup first.
    async fn handle_state_message(&self, topic: Topic, message: PhotonMessage, peer_id: &Id) -> Result<()> {
        if !matches!(topic, Topic::UserInbox) {
            return Err(EncodingError::new("Invalid home-peer notification"));
        }
        let from = message
            .from()
            .copied()
            .ok_or_else(|| EncodingError::new("Incoming message has no sender"))?;
        if from == *peer_id && message.recipient == *self.user_id() {
            return self.handle_notification(message.payload, &from);
        }

        let Value::Bytes(payload) = message.payload else {
            return Err(EncodingError::new("Channel notification is not binary"));
        };
        // A notification addressed to its own sender is a channel event, and
        // mutating channel state must not interleave with a pending request.
        let _request = if message.recipient == from {
            Some(self.requests.lock().await)
        } else {
            None
        };
        let plaintext = if message.recipient == *self.user_id() {
            self.user_identity.decrypt_into(&from, &payload)?
        } else if message.recipient == from {
            let session = self.friend_sessions.lock().unwrap().get(&from).cloned()
                .ok_or_else(|| StateError::new("Channel notification session is missing"))?;
            session.decrypt_into(&from, &payload)
                .map_err(|error| AuthenticationError::new(format!(
                    "Decrypting channel notification envelope failed: {error}"
                )))?
        } else {
            return Err(EncodingError::new("Invalid channel notification recipient"));
        };
        self.handle_notification(Value::Bytes(plaintext), &from)
    }

    /// Handles a friend handshake.
    ///
    /// Handshakes travel through the outbox as well as the inbox: the copy we
    /// sent comes back to us, which is how we learn the service accepted it.
    async fn handle_handshake(&self, topic: Topic, message: PhotonMessage, from: Id) -> Result<()> {
        let timestamp = i64::try_from(message.created_at)
            .map_err(|_| EncodingError::new("Message timestamp is out of range"))?;
        let originated_here = message.id == Self::message_id(self.device_identity.id(), timestamp)?;
        if matches!(topic, Topic::UserOutbox) && originated_here {
            return Ok(());
        }
        let friend_id = if matches!(topic, Topic::UserOutbox) {
            message.recipient
        } else {
            from
        };
        let is_inbox = matches!(topic, Topic::UserInbox);
        if (is_inbox && message.recipient != *self.user_identity.id())
            || (!is_inbox && from != *self.user_identity.id())
        {
            return Err(EncodingError::new("Handshake is not addressed to this user"));
        }

        let encrypted_handshake = match message.payload {
            Value::Bytes(bytes) => bytes,
            _ => return Err(EncodingError::new("Handshake payload is not binary")),
        };
        let handshake_bytes = self
            .user_identity
            .decrypt_into(&friend_id, &encrypted_handshake)
            .map_err(|error| AuthenticationError::new(format!("Decrypting handshake failed: {error}")))?;
        let mut handshake: Handshake = decode_cbor_bytes(&handshake_bytes, "handshake")?;
        handshake.from = Some(from);

        match handshake.body {
            HandshakeType::FriendRequest(hello) => {
                self.on_friend_request(
                    friend_id,
                    if is_inbox { from } else { *self.user_identity.id() },
                    hello.unwrap_or_default(),
                    handshake.timestamp,
                    is_inbox,
                );
                Ok(())
            }
            HandshakeType::FriendRequestAccept(session_key) => {
                self.accept_friend_handshake(friend_id, session_key, handshake.timestamp, is_inbox)
                    .await
            }
        }
    }

    /// Completes an acceptance we did not initiate: the peer said yes.
    async fn accept_friend_handshake(
        &self,
        friend_id: Id,
        session_key: Vec<u8>,
        timestamp: i64,
        is_inbox: bool,
    ) -> Result<()> {
        let _request = self.requests.lock().await;
        if self.friend_requests.borrow().get(&friend_id)
            .is_some_and(|request| request.is_accepted())
        {
            debug!("Ignoring duplicate friend request acceptance from {friend_id}");
            return Ok(());
        }
        let initiator_id = if is_inbox { *self.user_identity.id() } else { friend_id };
        let request = self.pending_friend_request(&friend_id, &initiator_id)?;
        if Self::timestamp(timestamp) < request.created_at() {
            return Err(EncodingError::new("Friend request acceptance predates the request"));
        }
        let session_identity = CryptoIdentity::try_from(session_key.as_slice())
            .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
        self.store_friend_contact(&friend_id, &session_identity).await?;
        self.friend_sessions.lock().unwrap().entry(friend_id).or_insert(session_identity);
        self.on_friend_request_accepted(friend_id, timestamp, is_inbox);
        Ok(())
    }

    fn handle_content_message(&self, topic: Topic, mut message: PhotonMessage, from: Id) -> Result<()> {
        let outbox = matches!(topic, Topic::UserOutbox);
        if outbox && from != *self.user_id() {
            return Err(EncodingError::new("Outbox message has an unexpected sender"));
        }
        let contact_id = if outbox || message.recipient != *self.user_id() { message.recipient } else { from };
        if self.contacts.borrow().get(&contact_id).is_some_and(|contact| contact.is_blocked()) {
            return Err(StateError::new("Message is from a blocked contact"));
        }
        if !outbox && message.recipient != *self.user_id()
            && !self.contacts.borrow().get(&contact_id).is_some_and(|contact| contact.contact_type() == ContactType::Channel)
        {
            return Err(EncodingError::new("Content message has an unexpected recipient"));
        }
        let encrypted_content = match &message.payload {
            Value::Bytes(bytes) => bytes.clone(),
            _ => return Err(EncodingError::new("Content payload is not binary")),
        };
        let sessions = self.friend_sessions.lock().unwrap();
        let session = sessions
            .get(&contact_id)
            .ok_or_else(|| StateError::new(format!("No friend session for {contact_id}")))?;
        let content_bytes = if outbox {
            self.user_identity.decrypt_into(session.id(), &encrypted_content)
        } else {
            session.decrypt_into(&from, &encrypted_content)
        }
            .map_err(|error| AuthenticationError::new(format!("Decrypting content failed: {error}")))?;
        let wire: MessageContent = decode_cbor_bytes(&content_bytes, "message content")?;
        let body = match (wire.format, wire.body) {
            (ContentFormat::Text, Value::Text(text)) => text.into_bytes(),
            (ContentFormat::Binary, Value::Bytes(bytes)) => bytes,
            _ => return Err(EncodingError::new("Unsupported message content body")),
        };
        let mut headers = HashMap::new();
        for (key, value) in wire.headers {
            headers.insert(
                key,
                serde_json::to_value(value)
                    .map_err(|error| EncodingError::new(format!("Malformed content header: {error}")))?,
            );
        }
        drop(sessions);
        message.from = Some(from);
        message.received_at = if outbox { None } else { Some(SystemTime::now()) };
        message.sent_at = if outbox { Some(SystemTime::now()) } else { None };
        message.payload_bytes = content_bytes;
        message.content = Some(crate::message::Content::_new(headers, body));
        let created_at = i64::try_from(message.created_at)
            .map_err(|_| EncodingError::new("Message timestamp is out of range"))?;
        let conversation_contact = self.contacts.borrow().get(&contact_id).cloned()
            .unwrap_or_else(|| Contact::new(contact_id, ContactType::Auto, created_at));
        update_conversation(&self.conversations, conversation_contact, message.clone())?;
        if outbox {
            if message.id != Self::message_id(self.device_id(), created_at)?
            {
                self.message_listener.on_sent(&message);
            }
        } else {
            self.message_listener.on_message(&message);
        }
        Ok(())
    }

    fn timestamp(timestamp: i64) -> SystemTime {
        if timestamp < 0 {
            return SystemTime::now();
        }
        UNIX_EPOCH + Duration::from_millis(timestamp as u64)
    }

    fn on_friend_request(
        &self,
        user_id: Id,
        initiator_id: Id,
        hello: String,
        timestamp: i64,
        notify: bool,
    ) {
        if timestamp < 0 {
            warn!("Ignoring friend request from {user_id} with a negative timestamp");
            return;
        }
        if let Some(contact) = self.contacts.borrow().get(&user_id) {
            if contact.is_blocked() || contact.contact_type() == ContactType::Friend
                || contact.contact_type() == ContactType::Channel
            {
                return;
            }
        }
        let created_at = Self::timestamp(timestamp);
        if self.friend_requests.borrow().get(&user_id)
            .is_some_and(|request| request.is_accepted() || request.created_at() >= created_at)
        {
            debug!("Ignoring duplicate or stale friend request from {user_id}");
            return;
        }
        self.friend_requests.borrow_mut().insert(user_id, FriendRequest::new(
            user_id,
            initiator_id,
            Some(hello.clone()),
            created_at,
        ));

        if notify {
            self.friend_request_listener
                .on_friend_request(&user_id, Some(&hello));
        }
    }

    fn on_friend_request_accepted(
        &self,
        user_id: Id,
        timestamp: i64,
        notify: bool,
    ) {
        if timestamp < 0 {
            warn!("Ignoring friend request acceptance from {user_id} with a negative timestamp");
            return;
        }

        let accepted_at = Self::timestamp(timestamp);
        {
            let mut requests = self.friend_requests.borrow_mut();
            let Some(request) = requests.get_mut(&user_id) else {
                return;
            };
            if request.is_accepted() || request.is_expired() {
                return;
            }
            let initiator_id = if notify { self.user_identity.id() } else { &user_id };
            if request.initiator_id() != initiator_id || accepted_at < request.created_at() {
                warn!("Ignoring friend request acceptance from {user_id} that does not match the pending request");
                return;
            }
            request.accept_at(accepted_at);
        }

        if self.contacts.borrow().get(&user_id).is_some_and(|contact| contact.contact_type() == ContactType::Friend) {
            if notify {
                self.friend_request_listener.on_friend_request_accepted(&user_id);
            }
            return;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        let mut contact = Contact::new(user_id, ContactType::Friend, now_ms);
        contact.set_revision(self.contacts_revision.get());
        self.contacts.borrow_mut().insert(user_id, contact.clone());
        if notify {
            self.friend_request_listener
                .on_friend_request_accepted(&user_id);
        }
        self.contact_listener.on_contact_added(&contact);
    }

    fn pending_friend_request(&self, user_id: &Id, initiator_id: &Id) -> Result<FriendRequest> {
        let requests = self.friend_requests.borrow();
        let request = requests.get(user_id).ok_or_else(|| {
            ArgumentError::new(format!("No friend request found for {user_id}"))
        })?;
        if self.contacts.borrow().get(user_id)
            .is_some_and(|contact| contact.is_blocked() || contact.contact_type() == ContactType::Channel)
        {
            return Err(ArgumentError::new("Cannot accept a friend request from a blocked user or channel"));
        }
        if request.initiator_id() != initiator_id {
            return Err(ArgumentError::new("Friend request has an unexpected initiator"));
        }
        if request.is_accepted() {
            return Err(ArgumentError::new("Friend request has already been accepted"));
        }
        if request.is_expired() {
            return Err(ArgumentError::new("Friend request has expired"));
        }
        Ok(request.clone())
    }

    /// Records a friend locally, including their session key.
    async fn store_friend_contact(&self, user_id: &Id, session: &CryptoIdentity) -> Result<()> {
        if self.contacts.borrow().get(user_id)
            .is_some_and(|contact| contact.contact_type() == ContactType::Friend)
        {
            return Ok(());
        }
        let revision = self.contacts_revision.get();
        let mut data = ContactData::new(*user_id, ContactType::Friend as u8, Self::current_timestamp()?);
        data.set_session_key(self.user_identity.encrypt_into(
            self.user_id(), session.signature_key().private_key().as_ref(),
        )?);
        let mutation = ContactMutation::add(
            revision,
            data.to_opaque(revision, &self.user_identity)?,
        );
        self.apply_contact_mutation(mutation, revision, "Friend contact revision is outdated")
            .await?;
        Ok(())
    }

    /// Sends a friend handshake to `user_id` and returns its timestamp.
    ///
    /// The handshake body is sealed for the friend, then the whole message is
    /// sealed again for the service, so only the service can see that a
    /// handshake exists and only the friend can read it.
    async fn send_handshake(
        &self,
        peer_id: &Id,
        user_id: Id,
        body: HandshakeType,
    ) -> Result<i64> {
        self.transport.ensure_open()?;
        let timestamp = Self::origin_timestamp(&self.origin_clock)?;
        let handshake = Handshake::new(timestamp, body)?;
        let handshake_bytes = to_cbor_bytes(&handshake, "handshake")?;
        let encrypted_handshake = self.user_identity
            .encrypt_into(&user_id, &handshake_bytes)
            .map_err(|error| EncodingError::new(format!("Encrypting handshake failed: {error}")))?;

        let message = PhotonMessage::new(
            MESSAGE_VERSION,
            Self::message_id(self.device_identity.id(), timestamp)?,
            user_id,
            HANDSHAKE_MESSAGE,
            None,
            timestamp as u64,
            Value::Bytes(encrypted_handshake),
        );
        let payload = mqtt::seal_envelope(&self.device_identity, peer_id, &message, "handshake envelope")?;
        self.transport.publish(payload, "handshake message").await?;
        Ok(timestamp)
    }

    fn message_id(device_id: &Id, timestamp: i64) -> Result<Id> {
        let mut digest = Sha256::new();
        digest.update(device_id.as_bytes());
        digest.update(timestamp.to_be_bytes());
        Id::try_from_bytes(digest.finalize().as_slice())
            .map_err(|error| EncodingError::new(error.to_string()).into())
    }

    /// Disconnects the transport and reports the state change to listeners.
    async fn stop_transport(&self) -> Result<()> {
        info!("Stopping messaging session...");
        if !self.transport.is_open() {
            return Ok(());
        }
        debug!("Disconnecting MQTT client...");
        let result = self.transport.disconnect().await;

        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
        info!("Messaging session stopped");
        result
    }

}

// ---------------------------------------------------------------------------
// Client API · conversations and messages
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Looks up a conversation by contact id.
    pub(crate) async fn get_conversation(
        &self,
        id: &Id
    ) -> Result<Option<Conversation>> {
        Ok(self.conversations.lock().unwrap().get(id).cloned())
    }

    /// Lists every known conversation, oldest activity first.
    pub(crate) async fn get_conversations(&self) -> Result<Vec<Conversation>> {
        let mut conversations = self.conversations.lock().unwrap().values().cloned().collect::<Vec<_>>();
        conversations.sort_by(|left, right| {
            left.updated_at().cmp(&right.updated_at()).then_with(|| left.id().cmp(right.id()))
        });
        Ok(conversations)
    }

    /// Drops a conversation locally; the peer is not notified.
    pub(crate) async fn remove_conversation(&self, id: &Id) -> Result<()> {
        self.conversations.lock().unwrap().remove(id);
        Ok(())
    }

    /// Drops several conversations locally.
    pub(crate) async fn remove_conversations(&self, ids: &[Id]) -> Result<()> {
        let mut conversations = self.conversations.lock().unwrap();
        for id in ids {
            conversations.remove(id);
        }
        Ok(())
    }

    /// Reads a page of history. Not implemented yet.
    pub(crate) async fn get_messages(
        &self,
        _conversation_id: &Id,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages"))
    }

    /// Reads history in a timestamp range. Not implemented yet.
    pub(crate) async fn get_messages_in_range(
        &self,
        _conversation_id: &Id,
        _begin: i64,
        _end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages_in_range"))
    }

    /// Removes a message. Not implemented yet.
    pub(crate) async fn remove_message(&self, _message_id: i64) -> Result<()> {
        Err(NotImplemented::new("remove_message"))
    }

    /// Removes messages by id. Not implemented yet.
    pub(crate) async fn remove_messages_by_ids(&self, _message_ids: &[i64]) -> Result<()> {
        Err(NotImplemented::new("remove_messages_by_ids"))
    }

    /// Clears a conversation's history. Not implemented yet.
    pub(crate) async fn remove_messages_in_conversation(&self, _conversation_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_messages_in_conversation"))
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        let result = self.rpc_call(self.peer_id(), Method::SessionList, None).await?;
        decode_cbor(result, "session list response")
    }

    /// Revokes another device's session.
    pub(crate) async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        if device_id == self.device_id() {
            return Err(ArgumentError::new("Cannot revoke the current session"));
        }
        let params = to_cbor(device_id, "session ID")?;
        let result = self.rpc_call(self.peer_id(), Method::SessionRevoke, Some(params)).await?;
        if result != Value::Null {
            return Err(EncodingError::new("Session revoke response contains an unexpected result"));
        }
        Ok(())
    }

}

// ---------------------------------------------------------------------------
// Client API · friends
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Sends a friend request and records it as pending.
    pub(crate) async fn friend_request(
        &self,
        user_id: &Id,
        hello: Option<String>,
    ) -> Result<()> {
        let _ = self.requests.lock().await;
        if user_id == self.user_identity.id() {
            return Err(ArgumentError::new(
                "Cannot send friend request to yourself",
            ));
        }

        info!("Sending friend request to {user_id}");
        let timestamp = self.send_handshake(
            self.options.peer_id(),
            *user_id,
            HandshakeType::FriendRequest(hello.clone()),
        ).await?;

        let request = FriendRequest::new(
            *user_id,
            *self.user_identity.id(),
            hello,
            Self::timestamp(timestamp),
        );
        self.friend_requests.borrow_mut().insert(*user_id, request);
        Ok(())
    }

    /// Accepts a pending friend request, generating a fresh session key.
    pub(crate) async fn accept_friend_request(
        &self,
        user_id: &Id
    ) -> Result<()> {
        let _request = self.requests.lock().await;

        let request = self.friend_requests.borrow().get(user_id).cloned()
            .ok_or_else(|| ArgumentError::new(format!("No friend request found for {user_id}")))?;
        if request.initiator_id() == self.user_identity.id() {
            return Err(ArgumentError::new("Can not accept your own friend request"));
        }
        if request.is_accepted() {
            return Err(ArgumentError::new("Friend request has already been accepted"));
        }
        if request.is_expired() {
            return Err(ArgumentError::new("Friend request has expired"));
        }

        if self.contacts
            .borrow()
            .get(user_id)
            .map(|v| v.is_blocked() || v.contact_type() == ContactType::Channel)
            .unwrap_or(false)
        {
            return Err(ArgumentError::new("Cannot accept a friend request from a blocked user or channel"));
        }

        let key = signature::KeyPair::random();
        let value = HandshakeType::FriendRequestAccept(key.private_key().as_bytes().to_vec());
        let session_identity = CryptoIdentity::from(key);
        self.store_friend_contact(user_id, &session_identity).await?;
        let timestamp = self.send_handshake(
            self.options.peer_id(),
            *user_id,
            value,
        ).await?;

        if Self::timestamp(timestamp) < request.created_at() {
            return Err(StateError::new("Friend request changed while accepting"));
        }

        self.friend_sessions.lock().unwrap().insert(*user_id, session_identity);
        self.on_friend_request_accepted(*user_id, timestamp, false);
        Ok(())
    }

    /// Looks up a single pending friend request.
    pub(crate) async fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> Result<Option<FriendRequest>> {
        let borrowed = self.friend_requests.borrow();
        let request = borrowed.get(user_id).cloned();
        Ok(request)
    }

    /// Lists every pending friend request.
    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let borrowed = self.friend_requests.borrow();
        let requests = borrowed.values().cloned().collect::<Vec<_>>();
        Ok(requests)
    }

    /// Dismisses a pending friend request without accepting it.
    pub(crate) async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        let _ = self.requests.lock().await;
        self.friend_requests.borrow_mut().remove(user_id);
        Ok(())
    }

    /// Dismisses several pending friend requests.
    pub(crate) async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        let _ = self.requests.lock().await;
        let mut borrowed_mut = self.friend_requests.borrow_mut();
        for user_id in user_ids {
            borrowed_mut.remove(user_id);
        }
        Ok(())
    }

    /// Dismisses every pending friend request.
    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let _ = self.requests.lock().await;
        self.friend_requests.borrow_mut().clear();
        Ok(())
    }

    /// Registers an existing friend locally, given their session key.
    pub(crate) async fn add_friend(
        &self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        let _requests = self.requests.lock().await;

        let session_identity = CryptoIdentity::try_from(session_key.as_slice())?;
        self.friend_sessions.lock().unwrap().insert(*user_id, session_identity);

        let now_ms = SystemTime::now();
        let mut data = ContactData::new(*user_id, ContactType::Friend as u8, boson_api::as_ms!(now_ms) as i64);
        data.remark = remark;
        data.revision = 1;
        let contact = Contact::try_from(data)?;

        self.contacts.borrow_mut().insert(*user_id, contact.clone());
        self.contact_listener.on_contact_added(&contact);
        Ok(())
    }

    /// Blocks a user. Not implemented yet.
    pub(crate) async fn block_user(
        &self,
        _user_id: &Id
    ) -> Result<Contact> {
        Err(NotImplemented::new("block_user"))
    }

}

// ---------------------------------------------------------------------------
// Client API · channels
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Creates a channel and stores it as a channel contact.
    pub(crate) async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announce: bool,
    ) -> Result<Box<dyn Channel>> {
       // let _requests = self.requests.lock().await;

        let session = CryptoIdentity::new();
        let private_key = session.signature_key().private_key().as_bytes();
        let encoded_key = self.user_identity.encrypt_into(
            self.user_id(),
            private_key,
        )?;

        let new_channel_info = NewChannelInfo::new(
            *session.id(),
            encoded_key,
            permission,
            Some(name.clone()),
            notice.clone(),
            announce,
        );

        let params = to_cbor(&new_channel_info, "channel creation request")?;

        let result = self.rpc_call(self.peer_id(), Method::ChannelCreate, Some(params)).await?;

        let (channel, returned_session) = self.decode_channel_info(result)?;
        if channel.owner != *self.user_id() || channel.session_id != *session.id() {
            return Err(EncodingError::new("Created channel owner or session does not match"));
        }
        self.store_channel(channel.clone(), returned_session).await?;
        self.channel_listener.on_channel_created(&channel);
        Ok(Box::new(channel))
    }

    /// Deletes a channel (owner only).
    pub(crate) async fn remove_channel(&self, channel_id: &Id) -> Result<()> {
        let _request = self.requests.lock().await;
        let channel = self.channel_snapshot(channel_id)?;
        if channel.owner != *self.user_id() {
            return Err(StateError::new("Only the owner can delete the channel"));
        }
        self.rpc_call(channel_id, Method::ChannelDelete, None).await?;
        self.remove_channel_contact(channel_id).await?;
        self.channel_listener.on_channel_deleted(&channel);
        Ok(())
    }

    /// Joins a channel from an invite ticket.
    pub(crate) async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<dyn Channel>> {
        let _request = self.requests.lock().await;
        if !ticket.is_genuine() || ticket.is_expired()
            || ticket.invitee().is_some_and(|id| id != self.user_id())
        {
            return Err(ArgumentError::new("Invalid, expired or incorrectly addressed channel ticket"));
        }
        if self.channels.borrow().get(ticket.channel_id()).is_some_and(|channel| channel.has_member(self.user_id())) {
            return Err(StateError::new("Already joined the channel"));
        }
        let key = if ticket.is_named_ticket() {
            self.user_identity.decrypt_into(ticket.inviter(), ticket.session_key())?
        } else {
            ticket.session_key().to_vec()
        };
        let session = CryptoIdentity::try_from(key.as_slice())?;
        if session.id() != ticket.session_id() {
            return Err(ArgumentError::new("Invitation session key does not match session ID"));
        }
        let key = self.user_identity.encrypt_into(self.user_id(), &key)?;
        let params = Value::Map([
            (Value::Text("c".into()), Value::Bytes(ticket.channel_id().as_bytes().to_vec())),
            (Value::Text("sid".into()), Value::Bytes(ticket.session_id().as_bytes().to_vec())),
            (Value::Text("i".into()), Value::Bytes(ticket.inviter().as_bytes().to_vec())),
            (Value::Text("e".into()), Value::Integer(ticket.expiration_ms().into())),
            (Value::Text("sig".into()), Value::Bytes(ticket.signature().to_vec())),
            (Value::Text("sk".into()), Value::Bytes(key)),
        ].into_iter().chain(ticket.invitee().map(|id|
            (Value::Text("ie".into()), Value::Bytes(id.as_bytes().to_vec()))
        )).collect());
        let result = self.rpc_call(ticket.channel_id(), Method::ChannelJoin, Some(params)).await?;
        let (channel, returned_session) = self.decode_channel_info(result)?;
        if channel.contact.id() != ticket.channel_id() || channel.session_id != *session.id()
            || !channel.has_member(self.user_id())
        {
            return Err(EncodingError::new("Joined channel does not match invitation or membership"));
        }
        self.store_channel(channel.clone(), returned_session).await?;
        self.channel_listener.on_joined_channel(&channel);
        Ok(Box::new(channel))
    }

    /// Leaves a channel. The owner cannot leave; they must delete it.
    pub(crate) async fn leave_channel(&self, channel_id: &Id) -> Result<()> {
        // let _request = self.requests.lock().await;
        let channel = self.channel_snapshot(channel_id)?;
        if channel.owner == *self.user_id() {
            return Err(StateError::new("The owner cannot leave the channel"));
        }
        self.rpc_call(channel_id, Method::ChannelLeave, None).await?;
        self.remove_channel_contact(channel_id).await?;
        self.channel_listener.on_left_channel(&channel);
        Ok(())
    }

    /// Mints an invite ticket, sealing the session key for the invitee.
    pub(crate) async fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        let _request = self.requests.lock().await;
        self.channel_snapshot(channel_id)?;
        let info = self.rpc_call(channel_id, Method::ChannelInfo, None).await?;
        let (channel, _) = self.decode_channel_info(info)?;
        if channel.id() != channel_id {
            return Err(EncodingError::new("Channel info has an unexpected ID"));
        }
        self.channels.borrow_mut().insert(*channel_id, channel.clone());
        let member = channel.member(self.user_id())
            .ok_or_else(|| StateError::new("Not a member of the channel"))?;
        let allowed = match channel.permission {
            Permission::Public | Permission::MemberInvite => !member.is_banned(),
            Permission::ModeratorInvite => member.is_owner() || member.is_moderator(),
            Permission::OwnerInvite => member.is_owner(),
        };
        if !allowed {
            return Err(StateError::new("Insufficient permission to invite channel members"));
        }
        if invitee.is_some_and(|id| channel.has_member(&id)) {
            return Err(ArgumentError::new("The invitee is already a channel member"));
        }
        let session = self.friend_sessions.lock().unwrap().get(channel_id).cloned()
            .ok_or_else(|| StateError::new("Channel session key is missing"))?;
        let key = session.signature_key().private_key().as_ref().to_vec();
        let key = match invitee {
            Some(id) => self.user_identity.encrypt_into(&id, &key)?,
            None => key,
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|error| StateError::new(format!("System clock error: {error}")))?;
        let expiration = u64::try_from(now.as_millis()).ok()
            .and_then(|now| now.checked_add(InviteTicket::DEFAULT_EXPIRATION_MS))
            .ok_or_else(|| StateError::new("Invitation expiration is out of range"))?;
        InviteTicket::create(&self.user_identity, *channel_id, channel.session_id, invitee, expiration, key)
    }

    fn channel_snapshot(&self, id: &Id) -> Result<PhotonChannel> {
        Ok(self.channels.borrow().get(id).cloned()
            .ok_or_else(|| StateError::new(format!("Channel {id} does not exist")))?)
    }

    fn decode_channel_info(&self, value: Value) -> Result<(PhotonChannel, CryptoIdentity)> {
        let info: ChannelInfo = decode_cbor(value, "channel info")?;
        self.channel_from_info(&info)
    }

    fn channel_from_info(&self, info: &ChannelInfo) -> Result<(PhotonChannel, CryptoIdentity)> {
        let timestamp = |time: SystemTime| -> Result<i64> {
            let millis = time.duration_since(UNIX_EPOCH)
                .map_err(|error| EncodingError::new(format!("Invalid channel timestamp: {error}")))?
                .as_millis();
            i64::try_from(millis).map_err(|_| EncodingError::new("Channel timestamp is out of range").into())
        };
        let created_at = timestamp(info.created_at())?;
        let updated_at = timestamp(info.updated_at())?;
        if updated_at < created_at {
            return Err(EncodingError::new("Invalid channel timestamps"));
        }
        for member in info.members() {
            validate_joined_at(member.joined())?;
        }
        let session = match info.session_key() {
            Some(key) => {
                let key = self.user_identity.decrypt_into(self.user_id(), key)?;
                CryptoIdentity::try_from(key.as_slice())?
            }
            None => self.friend_sessions.lock().unwrap().get(info.channel_id()).cloned()
                .ok_or_else(|| StateError::new("Channel session key is missing"))?,
        };
        if session.id() != info.session_id() {
            return Err(EncodingError::new("Channel session key does not match session ID"));
        }
        let mut data = ContactData::new(*info.channel_id(), ContactType::Channel as u8, created_at);
        data.name = info.name().map(ToString::to_string);
        data.updated_at = updated_at;
        let channel = PhotonChannel {
            contact: Contact::try_from(data)?,
            owner: *info.owner_id(), session_id: *info.session_id(), permission: info.permission(),
            notice: info.notice().map(ToString::to_string), announced: info.announce(),
            members: info.members().to_vec(),
        };
        let mut ids = HashSet::new();
        if channel.members.iter().any(|member| !ids.insert(*member.id()))
            || !channel.member(&channel.owner).is_some_and(|member| member.is_owner())
        {
            return Err(EncodingError::new("Channel members or owner are inconsistent"));
        }
        Ok((channel, session))
    }

    async fn store_channel(&self, mut channel: PhotonChannel, session: CryptoIdentity) -> Result<()> {
        let key = self.user_identity.encrypt_into(self.user_id(), session.signature_key().private_key().as_ref())?;
        let mut data = ContactData::new(*channel.id(), ContactType::Channel as u8, channel.contact.created_at());
        data.set_session_key(key.clone());
        data.name = channel.contact.name().map(ToString::to_string);
        data.updated_at = channel.contact.updated_at();
        let Value::Map(mut document) = to_cbor(&data, "channel contact")? else {
            return Err(EncodingError::new("Channel contact is not a map"));
        };
        insert_channel_fields(&mut document, &channel, key);
        let document = to_cbor_bytes(&Value::Map(document), "channel contact")?;
        let document = self.user_identity.encrypt_into(self.user_id(), &document)?;

        let revision = self.contacts_revision.get();
        let params = Value::Map([
            (Value::Text(FIELD_VERSION.into()), Value::Integer(revision.into())),
            (Value::Text(FIELD_OPERATION.into()), Value::Text("a".into())),
            (Value::Text(FIELD_DATA.into()), Value::Map([
                (Value::Text(FIELD_ID.into()), Value::Bytes(channel.id().as_bytes().to_vec())),
                (Value::Text(FIELD_VERSION.into()), Value::Integer(revision.into())),
                (Value::Text(FIELD_DATA.into()), Value::Bytes(document)),
            ].into_iter().collect())),
        ].into_iter().collect());
        let revision = self
            .commit_contact_mutation(params, revision, "Channel contact revision is outdated")
            .await?;
        channel.contact.set_revision(revision);
        self.friend_sessions.lock().unwrap().insert(*channel.id(), session);
        self.contacts.borrow_mut().insert(*channel.id(), channel.contact.clone());
        self.contact_listener.on_contact_added(&channel.contact);
        self.channels.borrow_mut().insert(*channel.id(), channel);
        Ok(())
    }

    async fn remove_channel_contact(&self, id: &Id) -> Result<()> {
        let revision = self.contacts_revision.get();
        let params = Value::Map([
            (Value::Text(FIELD_VERSION.into()), Value::Integer(revision.into())),
            (Value::Text(FIELD_OPERATION.into()), Value::Text("r".into())),
            (Value::Text(FIELD_DATA.into()), Value::Array(vec![Value::Bytes(id.as_bytes().to_vec())])),
        ].into_iter().collect());
        self.commit_contact_mutation(params, revision, "Channel contact revision is outdated")
            .await?;
        self.channels.borrow_mut().remove(id);
        self.contacts.borrow_mut().remove(id);
        self.friend_sessions.lock().unwrap().remove(id);
        self.contact_listener.on_contacts_removed(&[*id]);
        Ok(())
    }

    /// Transfers ownership. Not implemented yet.
    pub(crate) async fn transfer_channel_ownership(
        &self,
        _channel_id: &Id,
        _new_owner: Id,
    ) -> Result<()> {
        Err(NotImplemented::new("transfer_channel_ownership"))
    }

    /// Rotates the channel session key (owner only).
    pub(crate) async fn rotate_channel_session_key(&self, channel_id: &Id) -> Result<()> {
        let _request = self.requests.lock().await;
        let mut channel = self.channel_snapshot(channel_id)?;
        if channel.owner != *self.user_id() {
            return Err(StateError::new("Only the channel owner can rotate the session key"));
        }
        let current_session = self.friend_sessions.lock().unwrap().get(channel_id).cloned()
            .ok_or_else(|| StateError::new("Channel session key is missing"))?;
        if current_session.id() != &channel.session_id {
            return Err(StateError::new("Channel session key does not match the channel"));
        }

        let keypair = signature::KeyPair::random();
        let encrypted_key = self.user_identity.encrypt_into(
            current_session.id(),
            keypair.private_key().as_ref(),
        )?;
        let session = CryptoIdentity::from(keypair.clone());
        let rotation = ChannelSessionKeyRotation::new(*session.id(), encrypted_key);
        let params = to_cbor(&rotation, "channel session rotation")?;
        self.rpc_call(channel_id, Method::ChannelSessionKeyRotate, Some(params)).await?;

        channel.session_id = *session.id();
        self.friend_sessions.lock().unwrap().insert(*channel_id, session);
        self.channels.borrow_mut().insert(*channel_id, channel.clone());
        self.channel_listener.on_channel_session_key_rotated(&channel);
        Ok(())
    }

    /// Pushes changed channel metadata, sending only the fields that differ.
    pub(crate) async fn update_channel_info(&self, channel: &dyn Channel) -> Result<()> {
        let _request = self.requests.lock().await;
        let mut current = self.channel_snapshot(channel.id())?;
        if current.owner != *self.user_id() {
            return Err(StateError::new("Only the channel owner can update channel information"));
        }

        let mut changes = BTreeMap::new();
        if current.permission != channel.permission() {
            changes.insert(
                Value::Text(FIELD_PERMISSION.into()),
                Value::Integer((channel.permission() as i32).into()),
            );
        }
        if current.contact.name() != channel.name() {
            changes.insert(
                Value::Text(FIELD_NAME.into()),
                channel.name().map(|name| Value::Text(name.to_string())).unwrap_or(Value::Null),
            );
        }
        if current.notice.as_deref() != channel.notice() {
            changes.insert(
                Value::Text(FIELD_NOTICE.into()),
                channel.notice().map(|notice| Value::Text(notice.to_string())).unwrap_or(Value::Null),
            );
        }
        if current.announced != channel.is_announced() {
            changes.insert(
                Value::Text(FIELD_ANNOUNCED.into()),
                Value::Bool(channel.is_announced()),
            );
        }
        if changes.is_empty() {
            return Err(ArgumentError::new("Channel information has not changed"));
        }

        self.rpc_call(
            channel.id(),
            Method::ChannelInfoUpdate,
            Some(Value::Map(changes)),
        ).await?;
        current.permission = channel.permission();
        current.contact.set_name(channel.name().map(ToString::to_string));
        current.notice = channel.notice().map(ToString::to_string);
        current.announced = channel.is_announced();
        current.contact.set_updated_at(Self::current_timestamp()?);
        self.contacts.borrow_mut().insert(*channel.id(), current.contact.clone());
        self.channels.borrow_mut().insert(*channel.id(), current.clone());
        self.channel_listener.on_channel_updated(&current);
        Ok(())
    }

    /// Assigns a role to a set of channel members.
    ///
    /// Ownership and banning are deliberately excluded: they have dedicated
    /// operations because they change who may administer the channel.
    pub(crate) async fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: &[Id],
        role: Role,
    ) -> Result<()> {
        ensure_members(members)?;
        if matches!(role, Role::Owner | Role::Banned) {
            return Err(ArgumentError::new(
                "Owner and Banned roles cannot be assigned with this operation",
            ));
        }
        let params = Value::Map([
            (Value::Text(FIELD_MEMBERS.into()), member_id_array(members)),
            (Value::Text(FIELD_ROLE.into()), Value::Integer((role as i32).into())),
        ].into_iter().collect());
        self.manage_channel_members(
            channel_id,
            Method::ChannelMembersRoleUpdate,
            params,
            MemberUpdate::Role(role),
        ).await
    }

    pub(crate) async fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        ensure_members(members)?;
        self.manage_channel_members(
            channel_id,
            Method::ChannelMembersBan,
            member_id_array(members),
            MemberUpdate::Ban,
        ).await
    }

    pub(crate) async fn unban_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id],
    ) -> Result<()> {
        ensure_members(members)?;
        self.manage_channel_members(
            channel_id,
            Method::ChannelMembersUnban,
            member_id_array(members),
            MemberUpdate::Unban,
        ).await
    }

    pub(crate) async fn remove_channel_members(
        &self,
        channel_id: &Id,
        members: &[Id],
    ) -> Result<()> {
        ensure_members(members)?;
        self.manage_channel_members(
            channel_id,
            Method::ChannelMembersRemove,
            member_id_array(members),
            MemberUpdate::Remove,
        ).await
    }

    /// Runs a member-management RPC and mirrors its outcome locally.
    ///
    /// All four member operations differ only in their request shape and in how
    /// the confirmed members are re-roled, so the bookkeeping lives here.
    async fn manage_channel_members(
        &self,
        channel_id: &Id,
        method: Method,
        params: Value,
        update: MemberUpdate,
    ) -> Result<()> {
        let _request = self.requests.lock().await;
        let mut channel = self.channel_snapshot(channel_id)?;
        Self::ensure_channel_moderator(&channel, self.user_id())?;

        let result = self.rpc_call(channel_id, method, Some(params)).await?;
        let touched: Vec<Id> = decode_cbor(result, "updated channel members")?;
        let changed = update.apply(&mut channel, &touched);
        self.channels.borrow_mut().insert(*channel_id, channel.clone());
        if changed.is_empty() {
            return Ok(());
        }
        match update {
            MemberUpdate::Role(_) => {
                self.channel_listener.on_channel_members_role_updated(&channel, &changed)
            }
            MemberUpdate::Ban => {
                self.channel_listener.on_channel_members_banned(&channel, &changed)
            }
            MemberUpdate::Unban => {
                self.channel_listener.on_channel_members_unbanned(&channel, &changed)
            }
            MemberUpdate::Remove => {
                self.channel_listener.on_channel_members_removed(&channel, &changed)
            }
        }
        Ok(())
    }

    fn ensure_channel_moderator(channel: &PhotonChannel, user_id: &Id) -> Result<()> {
        if !channel.member(user_id).is_some_and(|member| member.is_owner() || member.is_moderator()) {
            return Err(StateError::new("Only a channel owner or moderator can manage members"));
        }
        Ok(())
    }

    fn current_timestamp() -> Result<i64> {
        let millis = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|error| StateError::new(format!("System clock error: {error}")))?
            .as_millis();
        i64::try_from(millis).map_err(|_| StateError::new("System clock timestamp is out of range").into())
    }

}

// ---------------------------------------------------------------------------
// Client API · contacts
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Looks up a single contact.
    pub(crate) async fn get_contact(&self, id: &Id) -> Result<Option<Contact>> {
        let _request = self.requests.lock().await;
        Ok(self.contacts.borrow().get(id).cloned())
    }

    /// Lists every known contact.
    pub(crate) async fn get_contacts(&self) -> Result<Vec<Contact>> {
        let _request = self.requests.lock().await;
        Ok(self.contacts.borrow().values().cloned().collect())
    }

    /// Pushes changed contact metadata as a contact mutation.
    pub(crate) async fn update_contact(&self, contact: &Contact) -> Result<()> {
        let _request = self.requests.lock().await;
        let origin = self.contacts.borrow().get(contact.id()).cloned()
            .ok_or_else(|| StateError::new("Contact not found"))?;
        if origin.remark() == contact.remark() && origin.tags() == contact.tags()
            && origin.is_muted() == contact.is_muted() && origin.is_blocked() == contact.is_blocked()
        {
            return Err(StateError::new("No changes detected"));
        }
        if contact.revision() < origin.revision() {
            return Err(StateError::new("Updated contact version is older than the current version"));
        }

        let revision = self.contacts_revision.get();
        let next_revision = revision.checked_add(1)
            .ok_or_else(|| StateError::new("Contact revision is exhausted"))?;
        let mut data = ContactData::from(origin.clone());
        data.remark = contact.remark().map(ToString::to_string);
        data.tags = contact.tags().map(ToString::to_string);
        data.muted = contact.is_muted();
        data.blocked = contact.is_blocked();
        data.updated_at = Self::current_timestamp()?.max(origin.updated_at());
        data.revision = next_revision;
        let session = self.friend_sessions.lock().unwrap().get(contact.id()).cloned();
        let key = session.as_ref().map(|session| self.user_identity.encrypt_into(
            self.user_id(), session.signature_key().private_key().as_ref(),
        )).transpose()?;
        if let Some(key) = &key {
            data.set_session_key(key.clone());
        }
        let mut updated = data.contact(next_revision)?;
        let opaque = if origin.contact_type() == ContactType::Channel {
            let channel = self.channel_snapshot(contact.id())?;
            let key = key.ok_or_else(|| StateError::new("Channel session key is missing"))?;
            let Value::Map(mut document) = to_cbor(&data, "channel contact")? else {
                return Err(EncodingError::new("Channel contact is not a map"));
            };
            insert_channel_fields(&mut document, &channel, key);
            let document = to_cbor_bytes(&Value::Map(document), "channel contact")?;
            let document = self.user_identity.encrypt_into(self.user_id(), &document)?;
            OpaqueContact::new(*contact.id(), next_revision, Value::Bytes(document))
        } else {
            data.to_opaque(next_revision, &self.user_identity)?
        };
        let mutation = ContactMutation::update(revision, opaque);
        let revision = self
            .apply_contact_mutation(mutation, next_revision, "Contact update revision is outdated")
            .await?;
        updated.set_revision(revision);
        self.contacts.borrow_mut().insert(*updated.id(), updated.clone());
        if let Some(channel) = self.channels.borrow_mut().get_mut(updated.id()) {
            channel.contact = updated.clone();
        }
        if updated.is_blocked() {
            self.blocked_contacts.lock().unwrap().insert(*updated.id());
        } else {
            self.blocked_contacts.lock().unwrap().remove(updated.id());
        }
        self.contact_listener.on_contacts_updated(&[updated]);
        Ok(())
    }

    /// Removes several contacts in one mutation.
    pub(crate) async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        if ids.is_empty() {
            return Err(ArgumentError::new("No contact IDs specified"));
        }

        let _request = self.requests.lock().await;
        let revision = self.contacts_revision.get();
        let mutation = ContactMutation::remove(revision, ids.to_vec());
        self.apply_contact_mutation(mutation, revision, "Contact removal revision is outdated")
            .await?;

        for id in ids {
            self.friend_sessions.lock().unwrap().remove(id);
            self.blocked_contacts.lock().unwrap().remove(id);
            self.contacts.borrow_mut().remove(id);
            self.channels.borrow_mut().remove(id);
        }
        self.contact_listener.on_contacts_removed(ids);
        Ok(())
    }


    /// Removes every contact in one mutation.
    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let _request = self.requests.lock().await;
        let revision = self.contacts_revision.get();
        let mutation = ContactMutation::clear(revision);
        self.apply_contact_mutation(mutation, revision, "Contact clear revision is outdated")
            .await?;

        self.friend_sessions.lock().unwrap().clear();
        self.blocked_contacts.lock().unwrap().clear();
        self.contacts.borrow_mut().clear();
        self.channels.borrow_mut().clear();
        self.contact_listener.on_contacts_cleared();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Lifecycle and event loop
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Polls the transport, holding off until a scheduled reconnect is due.
    pub(crate) async fn receive_event(&self) -> TransportEvent {
        if let Some(deadline) = self.reconnect_at.get() {
            tokio::time::sleep_until(deadline).await;
        }
        self.transport.poll().await
    }

    /// Queues a contact resync if one is due and none is already running.
    pub(crate) fn schedule_contact_sync(
        self: &Rc<Self>,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        if self.is_running() && self.is_ready() && !self.sync_running.get()
            && self.contact_sync_needed.replace(false)
        {
            self.sync_running.set(true);
            let session = self.clone();
            pending.push(async move {
                if let Err(error) = session.sync_contacts().await {
                    warn!("Contact resynchronization failed: {error}");
                }
                session.sync_running.set(false);
            }.boxed_local());
        }
    }

    /// Dispatches one event from the transport.
    ///
    /// The transport has already reduced the raw `rumqttc` event to an
    /// [`Inbound`], or dropped it when the RPC server has no interest in it.
    pub(crate) fn handle_mqtt_event(
        self: &Rc<Self>,
        event: TransportEvent,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        let Some(inbound) = self.transport.interpret(event) else {
            return;
        };
        match inbound {
            Inbound::Connected { session_present, accepted } => {
                self.on_connack(accepted, session_present, pending)
            }
            Inbound::Subscribed { accepted } => self.on_suback(accepted),
            Inbound::Message { topic, message } => {
                let session = self.clone();
                let name = topic.as_str();
                pending.push(async move {
                    if let Err(error) = session.dispatch(topic, *message).await {
                        warn!("Failed to process MQTT message on '{}': {error}", name);
                    }
                }.boxed_local());
            }
            Inbound::Unusable { topic, reason } => {
                warn!("Ignoring MQTT message on '{}': {reason}", topic);
            }
            Inbound::Failed { error } => self.on_transport_error(&error),
        }
    }

    /// Handles the service accepting the connection: subscribe to the topics.
    fn on_connack(
        self: &Rc<Self>,
        accepted: bool,
        session_present: bool,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        if !accepted {
            error!("Messaging MQTT connection was rejected");
            self.close_transport("Messaging MQTT connection rejected");
            return;
        }

        // Subscriptions are per-session, so readiness restarts on every ConnAck.
        self.ready.set(false);
        info!(
            "Connected to messaging server (session_present: {})",
            session_present
        );
        if !self.connected.replace(true) {
            self.notify_connection(|l| l.on_connected());
        }
        debug!(
            "Subscribing to topics: [{}, {}, {}]",
            Topic::UserInbox.as_str(), Topic::UserOutbox.as_str(), Topic::DeviceInbox.as_str()
        );

        let transport = self.transport.clone();
        let session = self.clone();
        pending.push(async move {
            if !transport.is_open() {
                error!("MQTT transport disappeared before subscription");
                session.close_transport("MQTT transport disappeared before subscription");
                return;
            }
            if let Err(error) = transport.subscribe_topics().await {
                error!("Failed to subscribe topics: {error}");
                session.close_transport("MQTT topic subscription failed");
            }
        }.boxed_local());
    }

    /// Handles the service confirming (or refusing) our subscriptions.
    fn on_suback(&self, accepted: bool) {
        if !accepted {
            error!("Messaging MQTT topic subscription was rejected");
            self.close_transport("Messaging MQTT topic subscription was rejected");
            return;
        }
        if !self.connected.replace(true) {
            self.notify_connection(|l| l.on_connected());
        }
    }

    /// Handles a transport failure: refresh credentials and schedule a retry.
    ///
    /// The password proves possession of the device key with a fresh nonce, so
    /// it has to be regenerated before every reconnect.
    fn on_transport_error(&self, error: &str) {
        self.ready.set(false);
        self.fail_pending_rpc(&format!("Messaging MQTT connection failed: {error}"));
        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
        if !self.running.get() {
            return;
        }

        match self.transport.password() {
            Ok(password) => self.transport.refresh_credentials(&password),
            Err(error) => {
                error!("Failed to refresh MQTT credentials: {error}");
                self.close_transport("Failed to refresh MQTT credentials");
                return;
            }
        }
        warn!("Messaging MQTT connection error: {error}");
        debug!("Waiting 2s before reconnecting...");
        self.reconnect_at.set(Some(tokio::time::Instant::now() + Duration::from_secs(2)));
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

    pub(crate) fn peer_endpoint(&self) -> &str {
        self.options.peer_endpoint().as_str()
    }

    pub(crate) fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    /// Connects the transport. Idempotent while already running.
    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stopping.set(false);
        if self.is_running() {
            debug!("Messaging session is already running");
            return Ok(());
        }

        let session = self.clone();
        let revision = self.contacts_revision.get();
        self.transport.connect(revision, move || {
            session.notify_connection(|listener| listener.on_connecting());
        }).await?;
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
        result
    }

    /// Whether the session is deployed and connected to the service.
    pub(crate) fn is_running(&self) -> bool {
        self.running.get()
    }

    /// Whether the broker handshake completed and traffic may be sent.
    pub(crate) fn is_connected(&self) -> bool {
        self.connected.get()
    }

    /// Whether the initial contact sync arrived, i.e. traffic may flow.
    pub(crate) fn is_ready(&self) -> bool {
        self.ready.get()
    }
}

// ---------------------------------------------------------------------------
// Message composition
// ---------------------------------------------------------------------------

impl RpcServer {
    /// Starts composing an outgoing message to `recipient`.
    pub(crate) fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(SessionMessageBuilder {
            recipient,
            client: self.transport.client_handle(),
            user_identity: self.user_identity.clone(),
            device_identity: self.device_identity.clone(),
            friend_sessions: self.friend_sessions.clone(),
            blocked_contacts: self.blocked_contacts.clone(),
            origin_clock: self.origin_clock.clone(),
            conversation_contact: recipient.and_then(|id| self.contacts.borrow().get(&id).cloned()),
            conversations: self.conversations.clone(),
            peer_id: *self.peer_id(),
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        })
    }
}

/// Collects a message on this side, then publishes it.
///
/// The session itself stays on the verticle thread, so the builder captures the
/// pieces it needs (identities, the MQTT handle, the conversation cache) and
/// does all encoding and encryption on the caller's thread.
struct SessionMessageBuilder {
    recipient: Option<Id>,
    client: ClientHandle,
    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,
    friend_sessions: Arc<std::sync::Mutex<HashMap<Id, CryptoIdentity>>>,
    blocked_contacts: Arc<std::sync::Mutex<HashSet<Id>>>,
    origin_clock: Arc<std::sync::Mutex<i64>>,
    conversation_contact: Option<Contact>,
    conversations: Arc<Mutex<HashMap<Id, Conversation>>>,
    peer_id: Id,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    text: bool,
}

impl MessageBuilder for SessionMessageBuilder {
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
            if self.blocked_contacts.lock().unwrap().contains(&recipient) {
                return Err(StateError::new("Cannot send a message to a blocked contact").into());
            }
            let mqtt: mqtt::Client = self.client.lock().unwrap().clone()
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;
            let friend_identity = self.friend_sessions.lock().unwrap().get(&recipient).cloned()
                .ok_or_else(|| StateError::new(format!("No friend session for {recipient}")))?;
            let body = self.body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let headers = build_message_headers(
                self.headers,
                self.content_type,
                self.content_disposition,
                self.text,
            );

            let timestamp = RpcServer::origin_timestamp(&self.origin_clock)?;
            let content = MessageContent {
                headers: headers_to_cbor(&headers)?,
                format: if self.text { ContentFormat::Text } else { ContentFormat::Binary },
                body: if self.text {
                    Value::Text(String::from_utf8(body.clone())
                        .map_err(|_| EncodingError::new("Text message is not UTF-8"))?)
                } else {
                    Value::Bytes(body.clone())
                },
            };
            let content_bytes = to_cbor_bytes(&content, "message content")?;
            let encrypted_content = self.user_identity
                .encrypt_into(friend_identity.id(), &content_bytes)
                .map_err(|error| AuthenticationError::new(format!("Encrypting content failed: {error}")))?;

            let created_at = u64::try_from(timestamp)
                .map_err(|_| EncodingError::new("Message timestamp is negative"))?;
            let mut wire = PhotonMessage::new(
                MESSAGE_VERSION,
                RpcServer::message_id(self.device_identity.id(), timestamp)?,
                recipient,
                CONTENT_MESSAGE,
                None,
                created_at,
                Value::Bytes(encrypted_content),
            );
            let payload = mqtt::seal_envelope(
                &self.device_identity, &self.peer_id, &wire, "message envelope",
            )?;
            mqtt::publish_outbox(&mqtt, payload, "message").await?;

            // The returned message mirrors what a recipient would see, so the
            // caller can render it without a round trip.
            wire.from = Some(*self.user_identity.id());
            wire.sent_at = Some(SystemTime::now());
            wire.payload_bytes = content_bytes;
            wire.content = Some(crate::message::Content::_new(headers, body));
            let contact = self.conversation_contact.unwrap_or_else(|| Contact::new(
                recipient, ContactType::Auto, timestamp,
            ));
            update_conversation(&self.conversations, contact, wire.clone())?;
            Ok(Box::new(wire) as Box<dyn Message>)
        })
    }
}

/// Folds the typed builder fields into the header map the wire format takes.
///
/// A non-text body defaults to `application/octet-stream`; an explicit content
/// type always wins, and so does an explicit content disposition.
fn build_message_headers(
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
    } else if !text && !merged.contains_key(content_type::HEADER_NAME) {
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

/// Converts JSON header values into their CBOR representation.
fn headers_to_cbor(
    headers: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, Value>> {
    headers
        .iter()
        .map(|(key, value)| {
            serde_cbor::value::to_value(value)
                .map(|value| (key.clone(), value))
                .map_err(|error| EncodingError::new(format!("Encoding header failed: {error}")).into())
        })
        .collect()
}

/// Folds a message into its conversation, creating the conversation on demand.
fn update_conversation(
    conversations: &Arc<Mutex<HashMap<Id, Conversation>>>,
    contact: Contact,
    message: PhotonMessage,
) -> Result<()> {
    let message = ConversationMessage {
        message,
        conversation_id: *contact.id(),
    };
    let mut conversations = conversations.lock().unwrap();
    let conversation = conversations.entry(*contact.id())
        .or_insert_with(|| Conversation::new(contact));
    conversation.update(Arc::new(message))
}

/// Pairs a wire message with the conversation it belongs to, so it can be
/// handed out as the public [`Message`] trait object.
struct ConversationMessage {
    message: PhotonMessage,
    conversation_id: Id,
}

impl Message for ConversationMessage {
    fn id(&self) -> &Id { self.message.id() }
    fn rid(&self) -> i64 { self.message.rid() }
    fn conversation_id(&self) -> Option<&Id> { Some(&self.conversation_id) }
    fn recipient(&self) -> &Id { self.message.recipient() }
    fn message_type(&self) -> crate::message::MessageType { self.message.message_type() }
    fn from(&self) -> Option<&Id> { self.message.from() }
    fn created_at(&self) -> SystemTime { self.message.created_at() }
    fn received_at(&self) -> Option<SystemTime> { self.message.received_at() }
    fn sent_at(&self) -> Option<SystemTime> { self.message.sent_at() }
    fn payload_as_bytes(&self) -> &[u8] { self.message.payload_as_bytes() }
    fn payload_as_content(&self) -> Option<&crate::message::Content> {
        self.message.payload_as_content()
    }
}
