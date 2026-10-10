use log::{debug, error, info, trace, warn};
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use rumqttc::{
    tokio_rustls::rustls::{
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
        ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
    },
    AsyncClient, Event, EventLoop, Incoming, MqttOptions, Publish, QoS, SubscribeFilter, TlsConfiguration,
    Transport,
};
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    path::Path,
    pin::Pin,
    rc::Rc,
    result::Result as StdResult,
    sync::Arc,
    future::Future,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::task;

use crate::{CryptoIdentity, Id, Identity};
use crate::errors::{Result, Error, ArgumentError, NotImplemented, StateError};
use crate::messaging::{
    options::Options,
    verticle::VerticleOptions,
    channel::{Channel, Permission, Role},
    contact::{Contact, ContactType},
    conversation::Conversation,
    friend_request::FriendRequest,
    invite_ticket::InviteTicket,
    message::{ContentDisposition, Message, MessageBuilder},
    session_info::SessionInfo,
    ChannelListener, ConnectionListener, ContactListener, MessageListener, SessionListener,
    FriendRequestListener,
    errors::{AuthenticationError, EncodingError, ProtocolError},
};

use super::internal::{PhotonContact, PhotonMessage};


const USER_INBOX: &str = "u/i";
const USER_OUTBOX: &str = "u/o";
const DEVICE_INBOX: &str = "d/i";
const DEVICE_OUTBOX: &str = "d/o";
const MAX_MESSAGE_SIZE: usize = 256 * 1024;
const MESSAGE_VERSION: u8 = 2;
const HANDSHAKE_MESSAGE: u8 = 0;
const RPC_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Serialize, Deserialize)]
struct RpcRequest {
    id: i64,
    #[serde(rename = "m")]
    method: String,
    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcResponse {
    id: i64,
    #[serde(rename = "m")]
    method: String,
    #[serde(rename = "r", default, skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcError {
    #[serde(rename = "c")]
    code: i32,
    #[serde(rename = "m")]
    message: String,
}

struct PendingRpc {
    method: String,
    complete: tokio::sync::oneshot::Sender<StdResult<Value, String>>,
}

struct RpcRegistration<'a> {
    pending: &'a RefCell<HashMap<i64, PendingRpc>>,
    id: i64,
}

impl Drop for RpcRegistration<'_> {
    fn drop(&mut self) {
        self.pending.borrow_mut().remove(&self.id);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Notification {
    id: Id,
    #[serde(rename = "s")]
    source: Id,
    #[serde(rename = "t")]
    timestamp: i64,
    #[serde(rename = "e")]
    event: String,
    #[serde(rename = "b", default, skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContactSync {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(rename = "t")]
    kind: u8,
    #[serde(rename = "d", default)]
    mutations: Vec<ContactMutation>,
    #[serde(rename = "s", default)]
    contacts: Vec<OpaqueContact>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContactMutation {
    #[serde(rename = "v")]
    revision: i32,
    op: String,
    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpaqueContact {
    id: Id,
    #[serde(rename = "v")]
    revision: i32,
    #[serde(rename = "d")]
    data: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContactData {
    id: Id,
    #[serde(rename = "t")]
    kind: u8,
    #[serde(rename = "sk", default)]
    session_key: Option<Value>,
    #[serde(rename = "n", default)]
    name: Option<String>,
    #[serde(rename = "r", default)]
    remark: Option<String>,
    #[serde(rename = "ts", default)]
    tags: Option<String>,
    #[serde(rename = "m", default)]
    muted: bool,
    #[serde(rename = "b", default)]
    blocked: bool,
    #[serde(rename = "c")]
    created_at: i64,
    #[serde(rename = "u")]
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireMessage {
    #[serde(rename = "v")]
    version: u8,
    id: Id,
    #[serde(rename = "r")]
    recipient: Id,
    #[serde(rename = "y")]
    message_type: u8,
    #[serde(rename = "f", skip_serializing_if = "Option::is_none")]
    from: Option<Id>,
    #[serde(rename = "c")]
    created_at: i64,
    #[serde(rename = "p")]
    payload: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum HandshakeType {
    #[serde(rename = "fr")]
    FriendRequest,
    #[serde(rename = "fra")]
    FriendRequestAccept,
}

#[derive(Debug, Serialize, Deserialize)]
struct Handshake {
    #[serde(rename = "t")]
    timestamp: i64,
    #[serde(rename = "y")]
    handshake_type: HandshakeType,
    #[serde(rename = "b")]
    body: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum ContentFormat {
    #[serde(rename = "t")]
    Text,
    #[serde(rename = "b")]
    Binary,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageContent {
    #[serde(rename = "h", skip_serializing_if = "HashMap::is_empty", default)]
    headers: HashMap<String, Value>,
    #[serde(rename = "f")]
    format: ContentFormat,
    #[serde(rename = "b")]
    body: Value,
}

#[derive(Debug)]
struct BosonServerCertVerifier {
    #[allow(dead_code)]
    expected_peer_id: Option<Id>,
}

impl ServerCertVerifier for BosonServerCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> StdResult<ServerCertVerified, RustlsError> {
        debug!(
            "TLS: verifying server certificate for {:?}, peer_id={:?}",
            server_name, self.expected_peer_id
        );
        // Accept self-signed / Boson peer certificates bound to the messaging node identity
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> StdResult<HandshakeSignatureValid, RustlsError> {
        rumqttc::tokio_rustls::rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> StdResult<HandshakeSignatureValid, RustlsError> {
        rumqttc::tokio_rustls::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub(crate) struct MqttSession {
    options: Arc<Options>,
    mqtt: Arc<std::sync::Mutex<Option<AsyncClient>>>,
    running: Cell<bool>,
    connected: Cell<bool>,
    ready: Cell<bool>,
    mqtt_task: RefCell<Option<task::JoinHandle<()>>>,
    lifecycle: tokio::sync::Mutex<()>,
    requests: tokio::sync::Mutex<()>,
    origin_clock: Arc<std::sync::Mutex<i64>>,
    pending_rpc: RefCell<HashMap<i64, PendingRpc>>,
    contacts_revision: Cell<i32>,
    contact_sync_needed: Cell<bool>,

    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,

    friend_sessions: Arc<std::sync::Mutex<HashMap<Id, CryptoIdentity>>>,
    blocked_contacts: Arc<std::sync::Mutex<HashSet<Id>>>,

    friend_requests: RefCell<HashMap<Id, FriendRequest>>,
    contacts: RefCell<HashMap<Id, PhotonContact>>,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn FriendRequestListener>,
}

// Compatibility names for the unchanged verticle references.
pub(crate) type Session = MqttSession;
pub(crate) type SessionAgent = MqttSession;

impl MqttSession {
    pub(crate) fn new(options: VerticleOptions) -> Result<Rc<Self>> {
        let user_key = options.options.user_key().clone();
        let user_identity = CryptoIdentity::from(user_key);

        let device_key = options.options.device_key().clone();
        let device_identity = CryptoIdentity::from(device_key);

        Ok(Rc::new(Self {
            options: options.options,
            mqtt: Arc::new(std::sync::Mutex::new(None)),
            running: Cell::new(false),
            connected: Cell::new(false),
            ready: Cell::new(false),
            mqtt_task: RefCell::new(None),
            lifecycle: tokio::sync::Mutex::new(()),
            requests: tokio::sync::Mutex::new(()),
            origin_clock: Arc::new(std::sync::Mutex::new(0)),
            pending_rpc: RefCell::new(HashMap::new()),
            contacts_revision: Cell::new(0),
            contact_sync_needed: Cell::new(false),
            user_identity,
            device_identity,
            friend_sessions: Arc::new(std::sync::Mutex::new(HashMap::new())),
            blocked_contacts: Arc::new(std::sync::Mutex::new(HashSet::new())),
            friend_requests: RefCell::new(HashMap::new()),
            contacts: RefCell::new(HashMap::new()),

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

    fn validate_outbox_packet(payload_len: usize) -> Result<()> {
        let remaining = payload_len.checked_add(2 + DEVICE_OUTBOX.len() + 2)
            .ok_or_else(|| ArgumentError::new("MQTT packet size is out of range"))?;
        let mut value = remaining;
        let mut header = 1;
        loop {
            header += 1;
            if value < 128 {
                break;
            }
            value /= 128;
        }
        if remaining > MAX_MESSAGE_SIZE.saturating_sub(header) {
            return Err(ArgumentError::new("Message exceeds the MQTT packet size limit"));
        }
        Ok(())
    }

    async fn rpc_call(&self, method: &str, params: Option<Value>) -> Result<Value> {
        if !self.connected.get() {
            return Err(StateError::new("Messaging client is not connected"));
        }
        let mqtt = self.mqtt.lock().unwrap().clone()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        let timestamp = Self::origin_timestamp(&self.origin_clock)?;
        let request = RpcRequest { id: timestamp, method: method.into(), params };
        let request = serde_cbor::to_vec(&request)
            .map_err(|error| EncodingError::new(format!("Encoding RPC request failed: {error}")))?;
        let message = WireMessage {
            version: MESSAGE_VERSION,
            id: Self::message_id(self.device_id(), timestamp)?,
            recipient: *self.peer_id(),
            message_type: crate::messaging::message::MessageType::ControlMessage as u8,
            from: None,
            created_at: timestamp,
            payload: Value::Bytes(request),
        };
        let message = serde_cbor::to_vec(&message)
            .map_err(|error| EncodingError::new(format!("Encoding RPC envelope failed: {error}")))?;
        let payload = self.device_identity.encrypt_into(self.peer_id(), &message)
            .map_err(|error| AuthenticationError::new(format!("Encrypting RPC envelope failed: {error}")))?;
        Self::validate_outbox_packet(payload.len())?;
        let (complete, response) = tokio::sync::oneshot::channel();
        self.pending_rpc.borrow_mut().insert(timestamp, PendingRpc { method: method.into(), complete });
        let _registration = RpcRegistration { pending: &self.pending_rpc, id: timestamp };
        tokio::time::timeout(RPC_TIMEOUT, async {
            mqtt.publish(DEVICE_OUTBOX, QoS::AtLeastOnce, false, payload).await
                .map_err(|error| StateError::new(format!("Publishing RPC request failed: {error}")))?;
            response.await
                .map_err(|_| StateError::new("RPC response channel closed"))?
                .map_err(|error| StateError::new(error).into())
        }).await.map_err(|_| StateError::new(format!("RPC '{method}' timed out")))?
    }

    fn fail_pending_rpc(&self, reason: &str) {
        let pending = std::mem::take(&mut *self.pending_rpc.borrow_mut());
        for (_, call) in pending {
            let _ = call.complete.send(Err(reason.into()));
        }
    }

    fn handle_rpc_response(&self, payload: Value) -> Result<()> {
        let Value::Bytes(payload) = payload else {
            return Err(EncodingError::new("RPC response payload is not binary"));
        };
        let response: RpcResponse = serde_cbor::from_slice(&payload)
            .map_err(|error| EncodingError::new(format!("Malformed RPC response: {error}")))?;
        let Some(call) = self.pending_rpc.borrow_mut().remove(&response.id) else {
            debug!("Ignoring RPC response {} without a pending call", response.id);
            return Ok(());
        };
        let result = if call.method != response.method {
            Err(format!("RPC response method '{}' does not match '{}'", response.method, call.method))
        } else if response.result.is_some() && response.error.is_some() {
            Err("RPC response contains both a result and an error".into())
        } else if let Some(error) = response.error {
            Err(format!("RPC '{}' failed ({}): {}", call.method, error.code, error.message))
        } else {
            Ok(response.result.unwrap_or(Value::Null))
        };
        if let Err(error) = &result {
            warn!("{error}");
        }
        let _ = call.complete.send(result);
        Ok(())
    }

    fn handle_notification(&self, payload: Value) -> Result<()> {
        let Value::Bytes(payload) = payload else {
            return Err(EncodingError::new("Notification payload is not binary"));
        };
        let notification: Notification = serde_cbor::from_slice(&payload)
            .map_err(|error| EncodingError::new(format!("Malformed notification: {error}")))?;
        if notification.timestamp < 0 {
            return Err(EncodingError::new("Notification timestamp is negative"));
        }
        let body = notification.body
            .ok_or_else(|| EncodingError::new("Notification body is missing"))?;
        match notification.event.as_str() {
            "sn" => {
                let session: SessionInfo = serde_cbor::value::from_value(body)
                    .map_err(|error| EncodingError::new(format!("Malformed session notification: {error}")))?;
                self.session_listener.on_new_session(&session);
            }
            "cs" => {
                let sync: ContactSync = serde_cbor::value::from_value(body)
                    .map_err(|error| EncodingError::new(format!("Malformed contact sync: {error}")))?;
                self.apply_contact_sync(sync)?;
            }
            event => return Err(EncodingError::new(format!("Unsupported home-peer notification '{event}'"))),
        }
        Ok(())
    }

    fn decode_contact(&self, opaque: OpaqueContact) -> Result<(PhotonContact, Option<CryptoIdentity>)> {
        if opaque.revision < 0 {
            return Err(EncodingError::new("Contact revision is negative"));
        }
        let Value::Bytes(encrypted) = opaque.data else {
            return Err(EncodingError::new("Opaque contact data is not binary"));
        };
        let plaintext = self.user_identity.decrypt_into(self.user_id(), &encrypted)
            .map_err(|error| AuthenticationError::new(format!("Decrypting contact failed: {error}")))?;
        let data: ContactData = serde_cbor::from_slice(&plaintext)
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?;
        if data.id != opaque.id || data.created_at < 0 || data.updated_at < data.created_at {
            return Err(EncodingError::new("Invalid contact identity or timestamps"));
        }
        let contact_type = match data.kind {
            0 => ContactType::Auto,
            1 => ContactType::Friend,
            2 => ContactType::Channel,
            _ => return Err(EncodingError::new("Unknown contact type")),
        };
        let session = match data.session_key {
            Some(Value::Bytes(encrypted)) => {
                let key = self.user_identity.decrypt_into(self.user_id(), &encrypted)
                    .map_err(|error| AuthenticationError::new(format!("Decrypting contact session key failed: {error}")))?;
                Some(CryptoIdentity::try_from(key.as_slice())
                    .map_err(|error| AuthenticationError::new(format!("Invalid contact session key: {error}")))?)
            }
            None if contact_type == ContactType::Auto => None,
            _ => return Err(EncodingError::new("Contact session key is missing or malformed")),
        };
        Ok((PhotonContact {
            id: data.id,
            contact_type,
            name: data.name,
            remark: data.remark,
            tags: data.tags,
            muted: data.muted,
            blocked: data.blocked,
            created_at: data.created_at,
            updated_at: data.updated_at,
            revision: opaque.revision,
        }, session))
    }

    fn apply_contact_sync(&self, sync: ContactSync) -> Result<()> {
        let original_revision = self.contacts_revision.get();
        if sync.revision < 0 {
            return Err(EncodingError::new("Contact sync revision is negative"));
        }
        let previous = self.contacts.borrow().clone();
        let mut contacts = previous.clone();
        let mut sessions = self.friend_sessions.lock().unwrap().clone();
        let mut revision = original_revision;
        match sync.kind {
            0 => {
                if !sync.contacts.is_empty() || !sync.mutations.is_empty() {
                    return Err(EncodingError::new("Up-to-date sync contains changes"));
                }
                if sync.revision != revision {
                    self.contact_sync_needed.set(sync.revision > revision);
                    return Err(StateError::new("Contact sync revision does not match local state"));
                }
                return Ok(());
            }
            2 => {
                if !sync.mutations.is_empty() || sync.revision < revision {
                    return Err(StateError::new("Invalid or outdated contact snapshot"));
                }
                contacts.clear();
                sessions.clear();
                for opaque in sync.contacts {
                    if opaque.revision > sync.revision {
                        return Err(EncodingError::new("Contact revision exceeds snapshot revision"));
                    }
                    let (contact, session) = self.decode_contact(opaque)?;
                    if contacts.contains_key(&contact.id) {
                        return Err(EncodingError::new("Snapshot contains duplicate contacts"));
                    }
                    if let Some(session) = session {
                        sessions.insert(contact.id, session);
                    }
                    contacts.insert(contact.id, contact);
                }
                revision = sync.revision;
            }
            1 => {
                if !sync.contacts.is_empty() || sync.mutations.is_empty() {
                    return Err(EncodingError::new("Invalid contact delta"));
                }
                for mutation in sync.mutations {
                    if mutation.revision < 0 {
                        return Err(EncodingError::new("Contact mutation revision is negative"));
                    }
                    if mutation.revision < revision {
                        continue;
                    }
                    if mutation.revision > revision {
                        self.contact_sync_needed.set(true);
                        return Err(StateError::new("Contact delta has a revision gap"));
                    }
                    let next = revision.checked_add(1)
                        .ok_or_else(|| EncodingError::new("Contact revision is exhausted"))?;
                    match mutation.op.as_str() {
                        "a" | "u" => {
                            let opaque: OpaqueContact = serde_cbor::value::from_value(mutation.data
                                .ok_or_else(|| EncodingError::new("Contact mutation data is missing"))?)
                                .map_err(|error| EncodingError::new(format!("Malformed contact mutation: {error}")))?;
                            if opaque.revision != next {
                                return Err(EncodingError::new("Contact mutation revision is inconsistent"));
                            }
                            let (contact, session) = self.decode_contact(opaque)?;
                            sessions.remove(&contact.id);
                            if let Some(session) = session {
                                sessions.insert(contact.id, session);
                            }
                            contacts.insert(contact.id, contact);
                        }
                        "r" => {
                            let ids: Vec<Id> = serde_cbor::value::from_value(mutation.data
                                .ok_or_else(|| EncodingError::new("Contact removal IDs are missing"))?)
                                .map_err(|error| EncodingError::new(format!("Malformed contact removal: {error}")))?;
                            for id in ids {
                                contacts.remove(&id);
                                sessions.remove(&id);
                            }
                        }
                        "c" => {
                            if mutation.data.is_some_and(|data| data != Value::Null) {
                                return Err(EncodingError::new("Contact clear contains unexpected data"));
                            }
                            contacts.clear();
                            sessions.clear();
                        }
                        _ => return Err(EncodingError::new("Unknown contact mutation operation")),
                    }
                    revision = next;
                }
                if revision < sync.revision || (revision > sync.revision && revision > original_revision) {
                    return Err(EncodingError::new("Contact delta final revision is inconsistent"));
                }
            }
            _ => return Err(EncodingError::new("Unknown contact sync type")),
        }
        let removed: Vec<_> = previous.keys().filter(|id| !contacts.contains_key(id)).copied().collect();
        let added: Vec<_> = contacts.values().filter(|contact| !previous.contains_key(&contact.id)).cloned().collect();
        let updated: Vec<Box<dyn Contact>> = contacts.values().filter(|contact| {
            previous.get(&contact.id).is_some_and(|old| old.revision != contact.revision
                || old.updated_at != contact.updated_at || old.blocked != contact.blocked
                || old.muted != contact.muted || old.name != contact.name || old.remark != contact.remark
                || old.tags != contact.tags || old.contact_type != contact.contact_type)
        }).cloned().map(|contact| Box::new(contact) as Box<dyn Contact>).collect();
        *self.blocked_contacts.lock().unwrap() = contacts.values()
            .filter(|contact| contact.blocked).map(|contact| contact.id).collect();
        *self.contacts.borrow_mut() = contacts;
        *self.friend_sessions.lock().unwrap() = sessions;
        self.contacts_revision.set(revision);
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

    async fn sync_contacts(&self) -> Result<()> {
        let result = self.rpc_call("cs", Some(Value::Integer(self.contacts_revision.get().into()))).await?;
        let sync: ContactSync = serde_cbor::value::from_value(result)
            .map_err(|error| EncodingError::new(format!("Malformed contact sync response: {error}")))?;
        self.apply_contact_sync(sync)
    }

    fn password(&self) -> Result<String> {
        let mut nonce = [0u8; 16];
        unsafe {
            libsodium_sys::randombytes_buf(nonce.as_mut_ptr() as *mut libc::c_void, 16);
        }

        let dsign = self.device_identity
            .sign_into(&nonce)
            .map_err(|error| AuthenticationError::new(error.to_string()))?;

        let mut password = Vec::with_capacity(nonce.len() + dsign.len());
        password.extend_from_slice(&nonce);
        password.extend_from_slice(&dsign);

        let base_password = bs58::encode(password).into_string();
        trace!(
            "Generated MQTT auth password for device {}",
            self.device_identity.id()
        );
        Ok(format!("{base_password}?contactsRevision={}", self.contacts_revision.get()))
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        callback(self.connection_listener.as_ref());
    }

    async fn start_mqtt(&self, options: &Options) -> Result<Option<(AsyncClient, EventLoop)>> {
        if self.is_running() {
            debug!("Messaging session is already running");
            return Ok(None);
        }

        let endpoint = options.peer_endpoint().clone();
        tokio::fs::create_dir_all(options.data_dir()).await?;

        let host = endpoint
            .host_str()
            .ok_or_else(|| ArgumentError::new("service endpoint has no hostname"))?;
        let port = endpoint
            .port()
            .ok_or_else(|| ArgumentError::new("service endpoint has no port"))?;

        self.notify_connection(|listener| listener.on_connecting());

        let client_id = self.device_identity.id().to_string();
        let mut mqtt_opts = MqttOptions::new(client_id.clone(), host.to_string(), port);
        mqtt_opts.set_credentials(self.user_identity.id().to_string(), self.password()?);
        mqtt_opts.set_keep_alive(Duration::from_secs(30));
        mqtt_opts.set_clean_session(false);
        mqtt_opts.set_max_packet_size(MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE);
        let is_tls = endpoint.scheme() == "mqtts" || endpoint.scheme() == "ssl" || port == 9083;
        if is_tls {
            let verifier = BosonServerCertVerifier {
                expected_peer_id: Some(*options.peer_id()),
            };
            let client_config = ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(verifier))
                .with_no_client_auth();
            mqtt_opts.set_transport(Transport::tls_with_config(TlsConfiguration::Rustls(
                Arc::new(client_config),
            )));
        }

        info!(
            "Connecting to messaging server at {} (TLS: {})",
            endpoint, is_tls
        );
        debug!(
            "MQTT client options: clientId={}, username={}, keepAlive=30s, cleanSession=false",
            client_id,
            self.user_identity.id()
        );

        let (mqtt, eventloop) = AsyncClient::new(mqtt_opts, 32);
        *self.mqtt.lock().unwrap() = Some(mqtt.clone());
        Ok(Some((mqtt, eventloop)))
    }

    fn handle_publish(&self, peer_id: &Id, publish: Publish) {
        debug!(
            "Processing publish message on topic '{}', payload size: {} bytes",
            publish.topic,
            publish.payload.len()
        );
        if publish.payload.len() > MAX_MESSAGE_SIZE {
            warn!("Ignoring oversized MQTT message on '{}'", publish.topic);
            return;
        }
        if publish.topic != USER_INBOX && publish.topic != USER_OUTBOX && publish.topic != DEVICE_INBOX {
            warn!("Ignoring MQTT message on unexpected topic '{}'", publish.topic);
            return;
        }

        if let Err(error) = self.handle_user_publish(peer_id, &publish.topic, publish.payload.as_ref()) {
            warn!("Failed to process MQTT message on '{}': {error}", publish.topic);
        }
    }

    fn handle_user_publish(&self, peer_id: &Id, topic: &str, payload: &[u8]) -> Result<()> {
        let envelope = self
            .device_identity
            .decrypt_into(peer_id, payload)
            .map_err(|error| AuthenticationError::new(format!("Decrypting message envelope failed: {error}")))?;
        let message: WireMessage = serde_cbor::from_slice(&envelope)
            .map_err(|error| EncodingError::new(format!("Malformed messaging envelope: {error}")))?;

        if message.version != MESSAGE_VERSION {
            return Err(ProtocolError::new(
                message.version as i32,
                "Unsupported message version",
            ));
        }
        let from = message
            .from
            .ok_or_else(|| EncodingError::new("Incoming message has no sender"))?;
        if message.created_at < 0 {
            return Err(EncodingError::new("Message timestamp is negative"));
        }
        if topic == DEVICE_INBOX {
            if message.message_type != crate::messaging::message::MessageType::ControlMessage as u8
                || from != *peer_id
                || (message.recipient != *self.user_id() && message.recipient != *self.device_id())
            {
                return Err(EncodingError::new("Invalid device RPC response"));
            }
            return self.handle_rpc_response(message.payload);
        }
        if message.message_type == crate::messaging::message::MessageType::StateMessage as u8 {
            if topic != USER_INBOX || from != *peer_id || message.recipient != *self.user_id() {
                return Err(EncodingError::new("Invalid home-peer notification"));
            }
            return self.handle_notification(message.payload);
        }
        if message.message_type != HANDSHAKE_MESSAGE {
            if message.message_type == crate::messaging::message::MessageType::ContentMessage as u8 {
                return self.handle_content_message(topic, message, from);
            }
            return Err(EncodingError::new("Unsupported user message type"));
        }

        let originated_here = message.id == Self::message_id(self.device_identity.id(), message.created_at)?;
        if topic == USER_OUTBOX && originated_here {
            return Ok(());
        }
        let friend_id = if topic == USER_OUTBOX {
            message.recipient
        } else {
            from
        };
        let is_inbox = topic == USER_INBOX;
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
        let handshake: Handshake = serde_cbor::from_slice(&handshake_bytes)
            .map_err(|error| EncodingError::new(format!("Malformed handshake: {error}")))?;
        if handshake.timestamp < 0 {
            return Err(EncodingError::new("Handshake timestamp is negative"));
        }

        match handshake.handshake_type {
            HandshakeType::FriendRequest => {
                let hello = match handshake.body {
                    Value::Text(hello) => hello,
                    _ => return Err(EncodingError::new("Friend request greeting is not text")),
                };
                self.on_friend_request(
                    friend_id,
                    if is_inbox { from } else { *self.user_identity.id() },
                    hello,
                    handshake.timestamp,
                    is_inbox,
                );
            }
            HandshakeType::FriendRequestAccept => {
                if self.friend_requests.borrow().get(&friend_id)
                    .is_some_and(|request| request.is_accepted())
                {
                    debug!("Ignoring duplicate friend request acceptance from {friend_id}");
                    return Ok(());
                }
                let initiator_id = if is_inbox { *self.user_identity.id() } else { friend_id };
                let request = self.pending_friend_request(&friend_id, &initiator_id)?;
                if Self::timestamp(handshake.timestamp) < request.created_at() {
                    return Err(EncodingError::new("Friend request acceptance predates the request"));
                }
                let session_key = match handshake.body {
                    Value::Bytes(session_key) => session_key,
                    _ => {
                        return Err(EncodingError::new(
                            "Friend request acceptance session key is not binary",
                        ))
                    }
                };
                if session_key.len() != crate::signature::PrivateKey::BYTES {
                    return Err(EncodingError::new(format!(
                        "Invalid friend session key length: {}",
                        session_key.len()
                    )));
                }
                let session_identity = CryptoIdentity::try_from(session_key.as_slice())
                    .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
                self.friend_sessions.lock().unwrap().entry(friend_id).or_insert(session_identity);
                self.on_friend_request_accepted(friend_id, handshake.timestamp, is_inbox);
            }
        }
        Ok(())
    }

    fn handle_content_message(&self, topic: &str, message: WireMessage, from: Id) -> Result<()> {
        let outbox = topic == USER_OUTBOX;
        if outbox && from != *self.user_id() {
            return Err(EncodingError::new("Outbox message has an unexpected sender"));
        }
        let contact_id = if outbox || message.recipient != *self.user_id() { message.recipient } else { from };
        if self.contacts.borrow().get(&contact_id).is_some_and(|contact| contact.blocked) {
            return Err(StateError::new("Message is from a blocked contact"));
        }
        if !outbox && message.recipient != *self.user_id()
            && !self.contacts.borrow().get(&contact_id).is_some_and(|contact| contact.contact_type == ContactType::Channel)
        {
            return Err(EncodingError::new("Content message has an unexpected recipient"));
        }
        let encrypted_content = match message.payload {
            Value::Bytes(bytes) => bytes,
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
        let wire: MessageContent = serde_cbor::from_slice(&content_bytes)
            .map_err(|error| EncodingError::new(format!("Malformed message content: {error}")))?;
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
        let created_at = UNIX_EPOCH + Duration::from_millis(message.created_at.max(0) as u64);
        drop(sessions);
        let message = PhotonMessage {
            id: message.id,
            recipient: message.recipient,
            from: Some(from),
            created_at,
            received_at: if outbox { None } else { Some(SystemTime::now()) },
            sent_at: if outbox { Some(SystemTime::now()) } else { None },
            payload: content_bytes,
            content: crate::messaging::message::Content::_new(headers, body),
        };
        if outbox {
            if message.id != Self::message_id(self.device_id(), message.created_at.duration_since(UNIX_EPOCH)
                .map_err(|error| EncodingError::new(format!("Invalid message timestamp: {error}")))?.as_millis() as i64)?
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
            if contact.blocked || contact.contact_type == ContactType::Friend
                || contact.contact_type == ContactType::Channel
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
            request.accept(accepted_at);
        }

        if self.contacts.borrow().get(&user_id).is_some_and(|contact| contact.contact_type == ContactType::Friend) {
            if notify {
                self.friend_request_listener.on_friend_request_accepted(&user_id);
            }
            return;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
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
            .is_some_and(|contact| contact.blocked || contact.contact_type == ContactType::Channel)
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

    async fn send_handshake(
        &self,
        peer_id: &Id,
        user_id: Id,
        handshake_type: HandshakeType,
        body: Value,
    ) -> Result<i64> {
        let mqtt = self
            .mqtt
            .lock().unwrap()
            .clone()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        let timestamp = Self::origin_timestamp(&self.origin_clock)?;
        let handshake = Handshake {
            timestamp,
            handshake_type,
            body,
        };
        let handshake_bytes = serde_cbor::to_vec(&handshake)
            .map_err(|error| EncodingError::new(format!("Encoding handshake failed: {error}")))?;
        let encrypted_handshake = self
            .user_identity
            .encrypt_into(&user_id, &handshake_bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting handshake failed: {error}")))?;

        let message_id = Self::message_id(self.device_identity.id(), timestamp)?;
        let message = WireMessage {
            version: MESSAGE_VERSION,
            id: message_id,
            recipient: user_id,
            message_type: HANDSHAKE_MESSAGE,
            from: None,
            created_at: timestamp,
            payload: Value::Bytes(encrypted_handshake),
        };
        let message_bytes = serde_cbor::to_vec(&message)
            .map_err(|error| EncodingError::new(format!("Encoding message failed: {error}")))?;
        let mqtt_payload = self
            .device_identity
            .encrypt_into(peer_id, &message_bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting message envelope failed: {error}")))?;
        Self::validate_outbox_packet(mqtt_payload.len())?;

        mqtt.publish(DEVICE_OUTBOX, QoS::AtLeastOnce, false, mqtt_payload)
            .await
            .map_err(|error| StateError::new(format!("Publishing handshake failed: {error}")))?;
        Ok(timestamp)
    }

    pub(crate) fn register_friend_session(&self, user_id: Id, session_key: &[u8]) -> Result<()> {
        if session_key.len() != crate::signature::PrivateKey::BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid friend session key length: {}",
                session_key.len()
            )));
        }
        let session_identity = CryptoIdentity::try_from(session_key)
            .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
        self.friend_sessions.lock().unwrap().insert(user_id, session_identity);
        Ok(())
    }

    fn message_id(device_id: &Id, timestamp: i64) -> Result<Id> {
        let mut digest = Sha256::new();
        digest.update(device_id.as_bytes());
        digest.update(timestamp.to_be_bytes());
        Id::try_from_bytes(digest.finalize().as_slice())
            .map_err(|error| EncodingError::new(error.to_string()).into())
    }

    async fn stop_mqtt(&self) -> Result<()> {
        info!("Stopping messaging session...");
        let Some(mqtt) = self.mqtt.lock().unwrap().take() else {
            return Ok(());
        };
        debug!("Disconnecting MQTT client...");
        let result = mqtt.disconnect().await
            .map_err(|error| -> Error {
                StateError::new(format!("Disconnecting MQTT client failed: {error}"))
            });

        self.notify_connection(|l| l.on_disconnected());
        info!("Messaging session stopped");
        result
    }

    async fn friend_accept(
        &self,
        peer_id: &Id,
        user_id: Id
    ) -> Result<(i64, CryptoIdentity)> {
        info!("Session: accepting friend request from {user_id}");
        let session_key = crate::signature::KeyPair::random()
            .private_key()
            .as_ref()
            .to_vec();
        let session_identity = CryptoIdentity::try_from(session_key.as_slice())
            .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
        let timestamp = self.send_handshake(
            peer_id,
            user_id,
            HandshakeType::FriendRequestAccept,
            Value::Bytes(session_key),
        )
        .await?;
        Ok((timestamp, session_identity))
    }

    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        info!("Session: rejecting friend request from {user_id}");
        Err(NotImplemented::new("friend_reject"))
    }

    pub(crate) async fn friend_remove(&self, user_id: Id) -> Result<()> {
        info!("Session: removing friend {user_id}");
        self.remove_contact(&user_id).await
    }

    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        debug!("Session: querying friend info for {user_id}");
        Err(NotImplemented::new("friend_info"))
    }

    pub(crate) async fn get_conversation(
        &self,
        _id: &Id
    ) -> Result<Option<Box<dyn Conversation>>> {
        Err(NotImplemented::new("get_conversation"))
    }

    pub(crate) async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
        Err(NotImplemented::new("get_conversations"))
    }

    pub(crate) async fn remove_conversation(&self, _id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_conversation"))
    }

    pub(crate) async fn remove_conversations(&self, _ids: &[Id]) -> Result<()> {
        Err(NotImplemented::new("remove_conversations"))
    }

    pub(crate) async fn get_messages(
        &self,
        _conversation_id: &Id,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages"))
    }

    pub(crate) async fn get_messages_in_range(
        &self,
        _conversation_id: &Id,
        _begin: i64,
        _end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages_in_range"))
    }

    pub(crate) async fn remove_message(&self, _message_id: i64) -> Result<()> {
        Err(NotImplemented::new("remove_message"))
    }

    pub(crate) async fn remove_messages_by_ids(&self, _message_ids: &[i64]) -> Result<()> {
        Err(NotImplemented::new("remove_messages_by_ids"))
    }

    pub(crate) async fn remove_messages_in_conversation(&self, _conversation_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_messages_in_conversation"))
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        let result = self.rpc_call("sl", None).await?;
        serde_cbor::value::from_value(result)
            .map_err(|error| EncodingError::new(format!("Malformed session list response: {error}")).into())
    }

    pub(crate) async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        if device_id == self.device_id() {
            return Err(ArgumentError::new("Cannot revoke the current session"));
        }
        let params = serde_cbor::value::to_value(device_id)
            .map_err(|error| EncodingError::new(format!("Encoding session ID failed: {error}")))?;
        let result = self.rpc_call("sr", Some(params)).await?;
        if result != Value::Null {
            return Err(EncodingError::new("Session revoke response contains an unexpected result"));
        }
        Ok(())
    }

    pub(crate) async fn friend_request(
        &self,
        user_id: &Id,
        hello: Option<String>,
    ) -> Result<()> {
        let _request = self.requests.lock().await;
        if user_id == self.user_identity.id() {
            return Err(ArgumentError::new(
                "Cannot send friend request to yourself",
            ));
        }

        info!("Sending friend request to {user_id} with greeting: '{}'", hello.as_deref().unwrap_or_default());
        let timestamp = self.send_handshake(
            self.options.peer_id(),
            *user_id,
            HandshakeType::FriendRequest,
            Value::Text(hello.clone().unwrap_or_default()),
        )
        .await?;

        let friend_req = FriendRequest::new(
            *user_id,
            *self.user_identity.id(),
            hello,
            Self::timestamp(timestamp),
        );
        self.friend_requests.borrow_mut().insert(*user_id, friend_req);
        Ok(())
    }

    pub(crate) async fn accept_friend_request(
        &self,
        user_id: &Id
    ) -> Result<()> {
        let _request = self.requests.lock().await;
        self.pending_friend_request(user_id, user_id)?;

        let (timestamp, session_identity) = self.friend_accept(self.options.peer_id(), *user_id).await?;
        let request = self.pending_friend_request(user_id, user_id)?;
        if Self::timestamp(timestamp) < request.created_at() {
            return Err(StateError::new("Friend request changed while accepting"));
        }
        self.friend_sessions.lock().unwrap().insert(*user_id, session_identity);
        self.on_friend_request_accepted(*user_id, timestamp, false);
        Ok(())
    }

    pub(crate) async fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> Result<Option<FriendRequest>> {
        let requests = self.friend_requests.borrow();
        let request = requests.get(user_id).cloned();
        Ok(request)
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let requests = self.friend_requests.borrow();
        let requests = requests.values().cloned().collect::<Vec<_>>();
        Ok(requests)
    }

    pub(crate) async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        let _request = self.requests.lock().await;
        self.friend_requests.borrow_mut().remove(user_id);
        Ok(())
    }

    pub(crate) async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        let _request = self.requests.lock().await;
        let mut requests = self.friend_requests.borrow_mut();
        for user_id in user_ids {
            requests.remove(user_id);
        }
        Ok(())
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let _request = self.requests.lock().await;
        self.friend_requests.borrow_mut().clear();
        Ok(())
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        let _request = self.requests.lock().await;
        self.register_friend_session(*user_id, &session_key)?;
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
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
        self.contacts.borrow_mut().insert(*user_id, contact.clone());
        self.contact_listener.on_contact_added(&contact);
        Ok(())
    }

    pub(crate) async fn block_user(
        &self,
        _user_id: &Id
    ) -> Result<Box<dyn Contact>> {
        Err(NotImplemented::new("block_user"))
    }

    pub(crate) async fn create_channel(
        &self,
        _permission: Permission,
        _name: String,
        _notice: Option<String>,
        _announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        Err(NotImplemented::new("create_channel"))
    }

    pub(crate) async fn remove_channel(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_channel"))
    }

    pub(crate) async fn join_channel(&self, _ticket: InviteTicket) -> Result<Box<dyn Channel>> {
        Err(NotImplemented::new("join_channel"))
    }

    pub(crate) async fn leave_channel(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("leave_channel"))
    }

    pub(crate) async fn create_invite_ticket(
        &self,
        _channel_id: &Id,
        _invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        Err(NotImplemented::new("create_invite_ticket"))
    }

    pub(crate) async fn transfer_channel_ownership(
        &self,
        _channel_id: &Id,
        _new_owner: Id,
    ) -> Result<()> {
        Err(NotImplemented::new("transfer_channel_ownership"))
    }

    pub(crate) async fn rotate_channel_session_key(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("rotate_channel_session_key"))
    }

    pub(crate) async fn update_channel_info(&self, _channel: &dyn Channel) -> Result<()> {
        Err(NotImplemented::new("update_channel_info"))
    }

    pub(crate) async fn set_channel_members_role(
        &self,
        _channel_id: &Id,
        _members: &[Id],
        _role: Role,
    ) -> Result<()> {
        Err(NotImplemented::new("set_channel_members_role"))
    }

    pub(crate) async fn ban_channel_members(&self, _channel_id: &Id, _members: &[Id]) -> Result<()> {
        Err(NotImplemented::new("ban_channel_members"))
    }

    pub(crate) async fn unban_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> Result<()> {
        Err(NotImplemented::new("unban_channel_members"))
    }

    pub(crate) async fn remove_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> Result<()> {
        Err(NotImplemented::new("remove_channel_members"))
    }

    pub(crate) async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        Ok(self.contacts
            .borrow()
            .get(id)
            .cloned()
            .map(|contact| Box::new(contact) as Box<dyn Contact>))
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        Ok(self.contacts
            .borrow()
            .values()
            .cloned()
            .map(|contact| Box::new(contact) as Box<dyn Contact>)
            .collect())
    }

    pub(crate) async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        let _request = self.requests.lock().await;
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
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or(0),
            revision: contact.revision() + 1,
        };
        self.contacts.borrow_mut().insert(updated.id, updated.clone());
        if updated.blocked {
            self.blocked_contacts.lock().unwrap().insert(updated.id);
        } else {
            self.blocked_contacts.lock().unwrap().remove(&updated.id);
        }
        self.contact_listener
            .on_contacts_updated(&[Box::new(updated) as Box<dyn Contact>]);
        Ok(())
    }

    pub(crate) async fn remove_contact(&self, id: &Id) -> Result<()> {
        let _request = self.requests.lock().await;
        self.friend_sessions.lock().unwrap().remove(id);
        self.blocked_contacts.lock().unwrap().remove(id);
        self.contacts.borrow_mut().remove(id);
        self.contact_listener.on_contacts_removed(&[*id]);
        Ok(())
    }

    pub(crate) async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        let _request = self.requests.lock().await;
        for id in ids {
            self.friend_sessions.lock().unwrap().remove(id);
            self.blocked_contacts.lock().unwrap().remove(id);
            self.contacts.borrow_mut().remove(id);
        }
        self.contact_listener.on_contacts_removed(ids);
        Ok(())
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let _request = self.requests.lock().await;
        self.friend_sessions.lock().unwrap().clear();
        self.blocked_contacts.lock().unwrap().clear();
        self.contacts.borrow_mut().clear();
        self.contact_listener.on_contacts_cleared();
        Ok(())
    }
}

impl MqttSession {
    async fn run_mqtt(self: Rc<Self>, mqtt: AsyncClient, mut eventloop: EventLoop) {
        debug!("MQTT event loop started");
        let mut syncs = FuturesUnordered::new();
        while self.running.get() {
            if syncs.is_empty() && self.contact_sync_needed.replace(false) {
                let session = self.clone();
                syncs.push(async move {
                    if let Err(error) = session.sync_contacts().await {
                        warn!("Contact resynchronization failed: {error}");
                    }
                }.boxed_local());
            }
            let event = tokio::select! {
                event = eventloop.poll() => event,
                Some(_) = syncs.next(), if !syncs.is_empty() => continue,
            };
            match event {
                Ok(Event::Incoming(Incoming::ConnAck(connack))) => {
                    if connack.code == rumqttc::ConnectReturnCode::Success {
                        self.ready.set(false);
                        info!(
                            "Connected to messaging server (session_present: {})",
                            connack.session_present
                        );
                        if !self.connected.replace(true) {
                            self.notify_connection(|l| l.on_connected());
                        }
                        debug!(
                            "Subscribing to topics: [{}, {}, {}]",
                            USER_INBOX, USER_OUTBOX, DEVICE_INBOX
                        );
                        let sub_res = mqtt
                            .subscribe_many([
                                SubscribeFilter::new(USER_INBOX.to_string(), QoS::AtLeastOnce),
                                SubscribeFilter::new(USER_OUTBOX.to_string(), QoS::AtLeastOnce),
                                SubscribeFilter::new(DEVICE_INBOX.to_string(), QoS::AtLeastOnce),
                            ])
                            .await;
                        if let Err(e) = sub_res {
                            warn!("Failed to subscribe topics: {e}");
                        } else {
                            debug!("Topic subscriptions requested");
                        }
                    } else {
                        error!("Messaging MQTT ConnAck error code: {:?}", connack.code);
                        self.running.set(false);
                    }
                }
                Ok(Event::Incoming(Incoming::SubAck(suback))) => {
                    debug!(
                        "Received SubAck for packet {:?}, return codes: {:?}",
                        suback.pkid, suback.return_codes
                    );
                    if suback.return_codes.len() != 3 || suback.return_codes.iter()
                        .any(|code| !matches!(code, rumqttc::SubscribeReasonCode::Success(_)))
                    {
                        error!("Messaging MQTT topic subscription was rejected");
                        self.running.set(false);
                        self.ready.set(false);
                        continue;
                    }
                    if !self.connected.replace(true) {
                        self.notify_connection(|l| l.on_connected());
                    }
                    if !self.ready.replace(true) {
                        info!("Messaging session is ready");
                        self.notify_connection(|l| l.on_ready());
                    }
                }
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    debug!(
                        "Received MQTT Publish on topic '{}', QoS: {:?}, payload size: {} bytes",
                        publish.topic,
                        publish.qos,
                        publish.payload.len()
                    );
                    self.handle_publish(self.options.peer_id(), publish);
                }
                Ok(Event::Incoming(Incoming::PubAck(puback))) => {
                    trace!("Received PubAck for packet id {}", puback.pkid);
                }
                Ok(Event::Incoming(Incoming::PingResp)) => {
                    trace!("Received PingResp from messaging server");
                }
                Ok(Event::Outgoing(outgoing)) => {
                    trace!("Sent outgoing MQTT packet: {:?}", outgoing);
                }
                Ok(other) => {
                    trace!("MQTT event: {:?}", other);
                }
                Err(error) => {
                    self.ready.set(false);
                    self.fail_pending_rpc(&format!("Messaging MQTT connection failed: {error}"));
                    if self.connected.replace(false) {
                        self.notify_connection(|l| l.on_disconnected());
                    }
                    if self.running.get() {
                        match self.password() {
                            Ok(password) => {
                                eventloop.mqtt_options.set_credentials(self.user_id().to_base58(), password);
                            }
                            Err(error) => {
                                error!("Failed to refresh MQTT credentials: {error}");
                                self.running.set(false);
                                continue;
                            }
                        }
                        warn!("Messaging MQTT connection error: {error}");
                        debug!("Waiting 2s before reconnecting...");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
        debug!("MQTT event loop exited");
        self.fail_pending_rpc("Messaging MQTT event loop stopped");
        self.mqtt.lock().unwrap().take();
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

    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        let transport = self.start_mqtt(&self.options).await?;
        if let Some((mqtt, eventloop)) = transport {
            self.running.set(true);
            let handle = task::spawn_local(self.clone().run_mqtt(mqtt, eventloop));
            *self.mqtt_task.borrow_mut() = Some(handle);
        }
        Ok(())
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.fail_pending_rpc("Messaging session stopped");
        let _request = self.requests.lock().await;
        let result = self.stop_mqtt().await;
        self.running.set(false);
        self.connected.set(false);
        self.ready.set(false);
        let handle = self.mqtt_task.borrow_mut().take();
        if let Some(handle) = handle {
            handle.abort();
            if let Err(error) = handle.await {
                if !error.is_cancelled() {
                    return Err(StateError::new(format!("MQTT event loop failed: {error}")));
                }
            }
        }
        result
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.get()
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.connected.get()
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.get()
    }

    pub(crate) fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(SessionMessageBuilder {
            recipient,
            mqtt: self.mqtt.clone(),
            user_identity: self.user_identity.clone(),
            device_identity: self.device_identity.clone(),
            friend_sessions: self.friend_sessions.clone(),
            blocked_contacts: self.blocked_contacts.clone(),
            origin_clock: self.origin_clock.clone(),
            peer_id: *self.peer_id(),
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        })
    }

}



struct SessionMessageBuilder {
    recipient: Option<Id>,
    mqtt: Arc<std::sync::Mutex<Option<AsyncClient>>>,
    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,
    friend_sessions: Arc<std::sync::Mutex<HashMap<Id, CryptoIdentity>>>,
    blocked_contacts: Arc<std::sync::Mutex<HashSet<Id>>>,
    origin_clock: Arc<std::sync::Mutex<i64>>,
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
            let mqtt = self.mqtt.lock().unwrap().clone()
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;
            let friend_identity = self.friend_sessions.lock().unwrap().get(&recipient).cloned()
                .ok_or_else(|| StateError::new(format!("No friend session for {recipient}")))?;
            let user_identity = self.user_identity;
            let device_identity = self.device_identity;
            let body = self.body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let mut headers = HashMap::new();
            for (key, value) in self.headers {
                headers.insert(key, serde_json::Value::String(value));
            }
            if let Some(content_type) = self.content_type {
                headers.insert("Content-Type".into(), serde_json::Value::String(content_type));
            } else if !self.text && !headers.contains_key("Content-Type") {
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
            let timestamp = MqttSession::origin_timestamp(&self.origin_clock)?;
            let content = MessageContent {
                headers: headers
                    .iter()
                    .map(|(key, value)| {
                        serde_cbor::value::to_value(value)
                            .map(|value| (key.clone(), value))
                            .map_err(|error| -> Error {
                                EncodingError::new(format!("Encoding header failed: {error}"))
                            })
                    })
                    .collect::<Result<HashMap<_, _>>>()?,
                format: if self.text { ContentFormat::Text } else { ContentFormat::Binary },
                body: if self.text {
                    Value::Text(String::from_utf8(body.clone())
                        .map_err(|_| EncodingError::new("Text message is not UTF-8"))?)
                } else {
                    Value::Bytes(body.clone())
                },
            };
            let content_bytes = serde_cbor::to_vec(&content)
                .map_err(|error| EncodingError::new(format!("Encoding message content failed: {error}")))?;
            let encrypted_content = user_identity
                .encrypt_into(friend_identity.id(), &content_bytes)
                .map_err(|error| AuthenticationError::new(format!("Encrypting content failed: {error}")))?;
            let message_id = MqttSession::message_id(device_identity.id(), timestamp)?;
            let wire = WireMessage {
                version: MESSAGE_VERSION,
                id: message_id,
                recipient,
                message_type: crate::messaging::message::MessageType::ContentMessage as u8,
                from: None,
                created_at: timestamp,
                payload: Value::Bytes(encrypted_content),
            };
            let message_bytes = serde_cbor::to_vec(&wire)
                .map_err(|error| EncodingError::new(format!("Encoding message failed: {error}")))?;
            let mqtt_payload = device_identity
                .encrypt_into(&self.peer_id, &message_bytes)
                .map_err(|error| AuthenticationError::new(format!("Encrypting message envelope failed: {error}")))?;
            MqttSession::validate_outbox_packet(mqtt_payload.len())?;
            mqtt.publish(DEVICE_OUTBOX, QoS::AtLeastOnce, false, mqtt_payload)
                .await
                .map_err(|error| StateError::new(format!("Publishing message failed: {error}")))?;

            Ok(Box::new(PhotonMessage {
                id: message_id,
                recipient,
                from: Some(*user_identity.id()),
                created_at: UNIX_EPOCH + Duration::from_millis(timestamp as u64),
                received_at: None,
                sent_at: Some(SystemTime::now()),
                payload: content_bytes,
                content: crate::messaging::message::Content::_new(headers, body),
            }) as Box<dyn Message>)
        })
    }
}
