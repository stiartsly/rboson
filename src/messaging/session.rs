use log::{debug, error, info, trace, warn};
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
    collections::HashMap,
    path::Path,
    pin::Pin,
    rc::{Rc, Weak},
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
    MessagingClient,
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

/// Session instance holding all necessary information to interact with the MQTT server.
/// Mutable session state is exclusively referenced within a single dedicated thread
/// running in a LocalSet and is managed by SessionAgent.
pub(crate) struct Session {
    mqtt: Option<AsyncClient>,

    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,

    friend_sessions: HashMap<Id, CryptoIdentity>,
    friend_requests: HashMap<Id, FriendRequest>,
    contacts: HashMap<Id, PhotonContact>,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn crate::messaging::FriendRequestListener>,
}

impl Session {
    pub(crate) fn new(vopts: VerticleOptions) -> Result<Rc<SessionAgent>> {
        let user_identity = CryptoIdentity::from(
            vopts.options.user_key().clone()
        );
        let device_identity = CryptoIdentity::from(
            vopts.options.device_key().clone()
        );

        let session = Self {
            mqtt: None,
            user_identity,
            device_identity,
            friend_sessions: HashMap::new(),
            friend_requests: HashMap::new(),
            contacts: HashMap::new(),

            connection_listener: vopts.connection_listener.clone(),
            message_listener: vopts.message_listener.clone(),
            channel_listener: vopts.channel_listener.clone(),
            contact_listener: vopts.contact_listener.clone(),
            session_listener: vopts.session_listener.clone(),
            friend_request_listener: vopts.friend_request_listener.clone(),
        };
        Ok(Rc::new_cyclic(|agent| SessionAgent {
            self_reference: agent.clone(),
            options: vopts.options,
            session: Arc::new(tokio::sync::Mutex::new(session)),
            running: Cell::new(false),
            connected: Cell::new(false),
            ready: Cell::new(false),
            mqtt_task: RefCell::new(None),
            connection_listener: vopts.connection_listener,
        }))
    }

    #[allow(dead_code)]
    pub(crate) fn is_running(&self) -> bool {
        self.mqtt.is_some()
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
        Ok(format!("{base_password}?contactsRevision=0"))
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        callback(self.connection_listener.as_ref());
    }

    async fn start_mqtt(&mut self, options: &Options) -> Result<Option<(AsyncClient, EventLoop)>> {
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
        self.mqtt = Some(mqtt.clone());
        Ok(Some((mqtt, eventloop)))
    }

    fn handle_publish(&mut self, peer_id: &Id, publish: Publish) {
        debug!(
            "Processing publish message on topic '{}', payload size: {} bytes",
            publish.topic,
            publish.payload.len()
        );
        if publish.topic != USER_INBOX && publish.topic != USER_OUTBOX {
            return;
        }

        if let Err(error) = self.handle_user_publish(peer_id, &publish.topic, publish.payload.as_ref()) {
            warn!("Failed to process MQTT message on '{}': {error}", publish.topic);
        }
    }

    fn handle_user_publish(&mut self, peer_id: &Id, topic: &str, payload: &[u8]) -> Result<()> {
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
        if message.message_type != HANDSHAKE_MESSAGE {
            if message.message_type == crate::messaging::message::MessageType::ContentMessage as u8 {
                return self.handle_content_message(topic, message, from);
            }
            return Ok(());
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

        let encrypted_handshake = match message.payload {
            Value::Bytes(bytes) => bytes,
            _ => return Err(EncodingError::new("Handshake payload is not binary")),
        };
        let handshake_bytes = self
            .user_identity
            .decrypt_into(&from, &encrypted_handshake)
            .map_err(|error| AuthenticationError::new(format!("Decrypting handshake failed: {error}")))?;
        let handshake: Handshake = serde_cbor::from_slice(&handshake_bytes)
            .map_err(|error| EncodingError::new(format!("Malformed handshake: {error}")))?;

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
                self.friend_sessions.insert(friend_id, session_identity);
                self.on_friend_request_accepted(friend_id, handshake.timestamp, is_inbox);
            }
        }
        Ok(())
    }

    fn handle_content_message(&self, topic: &str, message: WireMessage, from: Id) -> Result<()> {
        if topic != USER_INBOX {
            return Ok(());
        }
        let encrypted_content = match message.payload {
            Value::Bytes(bytes) => bytes,
            _ => return Err(EncodingError::new("Content payload is not binary")),
        };
        let session = self.friend_sessions
            .get(&from)
            .ok_or_else(|| StateError::new(format!("No friend session for {from}")))?;
        let content_bytes = session
            .decrypt_into(&from, &encrypted_content)
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
        self.message_listener.on_message(&PhotonMessage {
            id: message.id,
            recipient: message.recipient,
            from: Some(from),
            created_at,
            received_at: Some(SystemTime::now()),
            sent_at: None,
            payload: content_bytes,
            content: crate::messaging::message::Content::_new(headers, body),
        });
        Ok(())
    }

    fn timestamp(timestamp: i64) -> SystemTime {
        if timestamp < 0 {
            return SystemTime::now();
        }
        UNIX_EPOCH + Duration::from_millis(timestamp as u64)
    }

    fn on_friend_request(
        &mut self,
        user_id: Id,
        initiator_id: Id,
        hello: String,
        timestamp: i64,
        notify: bool,
    ) {
        if let Some(contact) = self.contacts.get(&user_id) {
            if contact.contact_type == ContactType::Friend
                || contact.contact_type == ContactType::Channel
            {
                return;
            }
        }
        self.friend_requests.insert(user_id, FriendRequest::new(
            user_id,
            initiator_id,
            Some(hello.clone()),
            Self::timestamp(timestamp),
        ));

        if notify {
            self.friend_request_listener
                .on_friend_request(&user_id, Some(&hello));
        }
    }

    fn on_friend_request_accepted(
        &mut self,
        user_id: Id,
        timestamp: i64,
        add_contact: bool,
    ) {
        let accepted_at = Self::timestamp(timestamp);
        {
            let Some(request) = self.friend_requests.get_mut(&user_id) else {
                return;
            };
            if request.is_accepted() || request.is_expired() {
                return;
            }
            if add_contact && request.initiator_id() == &user_id {
                return;
            }
            request.accept(accepted_at);
        }
        if !add_contact {
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
        self.contacts.insert(user_id, contact.clone());
        self.friend_request_listener
            .on_friend_request_accepted(&user_id);
        self.contact_listener.on_contact_added(&contact);
    }

    async fn send_handshake(
        &self,
        peer_id: &Id,
        user_id: Id,
        handshake_type: HandshakeType,
        body: Value,
    ) -> Result<()> {
        let mqtt = self
            .mqtt
            .clone()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| StateError::new(format!("System clock error: {error}")))?
            .as_millis() as i64;
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

        mqtt.publish(DEVICE_OUTBOX, QoS::AtLeastOnce, false, mqtt_payload)
            .await
            .map_err(|error| StateError::new(format!("Publishing handshake failed: {error}")).into())
    }

    pub(crate) fn register_friend_session(&mut self, user_id: Id, session_key: &[u8]) -> Result<()> {
        if session_key.len() != crate::signature::PrivateKey::BYTES {
            return Err(ArgumentError::new(format!(
                "Invalid friend session key length: {}",
                session_key.len()
            )));
        }
        let session_identity = CryptoIdentity::try_from(session_key)
            .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
        self.friend_sessions.insert(user_id, session_identity);
        Ok(())
    }

    fn message_id(device_id: &Id, timestamp: i64) -> Result<Id> {
        let mut digest = Sha256::new();
        digest.update(device_id.as_bytes());
        digest.update(timestamp.to_be_bytes());
        Id::try_from_bytes(digest.finalize().as_slice())
            .map_err(|error| EncodingError::new(error.to_string()).into())
    }

    async fn stop_mqtt(&mut self) -> Result<()> {
        info!("Stopping messaging session...");
        let mqtt = self.mqtt.take();
        let result = if let Some(mqtt) = mqtt {
            debug!("Disconnecting MQTT client...");
            mqtt.disconnect().await
                .map_err(|error| -> Error {
                    StateError::new(format!("Disconnecting MQTT client failed: {error}"))
                })
        } else {
            Ok(())
        };

        self.notify_connection(|l| l.on_disconnected());
        info!("Messaging session stopped");
        result
    }

    async fn send_friend_request(&self, peer_id: &Id, user_id: Id, hello: String) -> Result<()> {
        if &user_id == self.user_identity.id() {
            return Err(ArgumentError::new(
                "Cannot send friend request to yourself",
            ));
        }
        info!(
            "Session: sending friend request to {user_id} with greeting: '{hello}'"
        );
        self.send_handshake(
            peer_id,
            user_id,
            HandshakeType::FriendRequest,
            Value::Text(hello),
        )
        .await
    }

    pub(crate) async fn friend_accept(&mut self, peer_id: &Id, user_id: Id) -> Result<()> {
        info!("Session: accepting friend request from {user_id}");
        let session_key = crate::signature::KeyPair::random()
            .private_key()
            .as_ref()
            .to_vec();
        let session_identity = CryptoIdentity::try_from(session_key.as_slice())
            .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
        self.send_handshake(
            peer_id,
            user_id,
            HandshakeType::FriendRequestAccept,
            Value::Bytes(session_key),
        )
        .await?;
        self.friend_sessions.insert(user_id, session_identity);
        Ok(())
    }

    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        info!("Session: rejecting friend request from {user_id}");
        Err(NotImplemented::new("friend_reject"))
    }

    pub(crate) async fn friend_remove(&mut self, user_id: Id) -> Result<()> {
        info!("Session: removing friend {user_id}");
        self.remove_contact(&user_id).await
    }

    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        debug!("Session: querying friend info for {user_id}");
        Err(NotImplemented::new("friend_info"))
    }

    async fn get_conversation(
        &self,
        _id: &Id
    ) -> Result<Option<Box<dyn Conversation>>> {
        Err(NotImplemented::new("get_conversation"))
    }

    async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
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

    pub(crate) async fn friend_request(
        &mut self,
        peer_id: &Id,
        user_id: &Id,
        hello: Option<String>
    ) -> Result<()> {
        self.send_friend_request(
            peer_id,
            *user_id,
            hello.clone().unwrap_or_default()
        ).await?;

        let now = SystemTime::now();
        self.friend_requests.insert(*user_id, FriendRequest::new(
            *user_id,
            *self.user_identity.id(),
            hello,
            now,
        ));
        Ok(())
    }

    pub(crate) async fn accept_friend_request(&mut self, peer_id: &Id, user_id: &Id) -> Result<()> {
        {
            let request = self.friend_requests.get(user_id).ok_or_else(|| {
                ArgumentError::new(format!("No friend request found for {user_id}"))
            })?;
            if request.initiator_id() == self.user_identity.id() {
                return Err(ArgumentError::new("Cannot accept your own friend request"));
            }
            if request.is_accepted() {
                return Err(ArgumentError::new("Friend request has already been accepted"));
            }
            if request.is_expired() {
                return Err(ArgumentError::new("Friend request has expired"));
            }
        }

        self.friend_accept(peer_id, *user_id).await?;
        let now = SystemTime::now();
        if let Some(request) = self.friend_requests.get_mut(user_id) {
            request.accept(now);
        }
        let now_ms = now
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        let contact = PhotonContact {
            id: *user_id,
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
        self.contacts.insert(*user_id, contact.clone());
        self.contact_listener.on_contact_added(&contact);
        Ok(())
    }

    pub(crate) async fn get_friend_request(
        &self,
        user_id: &Id,
    ) -> Result<Option<FriendRequest>> {
        Ok(self.friend_requests
            .get(user_id)
            .cloned())
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        Ok(self.friend_requests
            .values()
            .cloned()
            .collect())
    }

    pub(crate) async fn remove_friend_request(&mut self, user_id: &Id) -> Result<()> {
        self.friend_requests.remove(user_id);
        Ok(())
    }

    pub(crate) async fn remove_friend_requests(&mut self, user_ids: &[Id]) -> Result<()> {
        for user_id in user_ids {
            self.friend_requests.remove(user_id);
        }
        Ok(())
    }

    pub(crate) async fn clear_friend_requests(&mut self) -> Result<()> {
        self.friend_requests.clear();
        Ok(())
    }

    pub(crate) async fn add_friend(
        &mut self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
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
        self.contacts.insert(*user_id, contact.clone());
        self.contact_listener.on_contact_added(&contact);
        Ok(())
    }

    pub(crate) async fn block_user(
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
        Ok(self.contacts
            .get(id)
            .cloned()
            .map(|contact| Box::new(contact) as Box<dyn Contact>))
    }

    async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        Ok(self.contacts
            .values()
            .cloned()
            .map(|contact| Box::new(contact) as Box<dyn Contact>)
            .collect())
    }

    async fn update_contact(&mut self, contact: &dyn Contact) -> Result<()> {
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
        self.contacts.insert(updated.id, updated.clone());
        self.contact_listener
            .on_contacts_updated(&[Box::new(updated) as Box<dyn Contact>]);
        Ok(())
    }

    async fn remove_contact(&mut self, id: &Id) -> Result<()> {
        self.friend_sessions.remove(id);
        self.contacts.remove(id);
        self.contact_listener.on_contacts_removed(&[*id]);
        Ok(())
    }

    async fn remove_contacts(&mut self, ids: &[Id]) -> Result<()> {
        for id in ids {
            self.friend_sessions.remove(id);
            self.contacts.remove(id);
        }
        self.contact_listener.on_contacts_removed(ids);
        Ok(())
    }

    async fn clear_contacts(&mut self) -> Result<()> {
        self.friend_sessions.clear();
        self.contacts.clear();
        self.contact_listener.on_contacts_cleared();
        Ok(())
    }
}

pub(crate) struct SessionAgent {
    self_reference: Weak<SessionAgent>,
    options: Arc<Options>,
    session: Arc<tokio::sync::Mutex<Session>>,
    running: Cell<bool>,
    connected: Cell<bool>,
    ready: Cell<bool>,
    mqtt_task: RefCell<Option<task::JoinHandle<()>>>,
    connection_listener: Arc<dyn ConnectionListener>,
}

impl SessionAgent {
    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        self.session.lock().await.friend_reject(user_id).await
    }

    pub(crate) async fn friend_remove(&self, user_id: Id) -> Result<()> {
        self.session.lock().await.friend_remove(user_id).await
    }

    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        self.session.lock().await.friend_info(user_id).await
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        callback(self.connection_listener.as_ref());
    }

    async fn run_mqtt(self: Rc<Self>, mqtt: AsyncClient, mut eventloop: EventLoop) {
        debug!("MQTT event loop started");
        while self.running.get() {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::ConnAck(connack))) => {
                    if connack.code == rumqttc::ConnectReturnCode::Success {
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
                    }
                }
                Ok(Event::Incoming(Incoming::SubAck(suback))) => {
                    debug!(
                        "Received SubAck for packet {:?}, return codes: {:?}",
                        suback.pkid, suback.return_codes
                    );
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
                    let agent = self.clone();
                    task::spawn_local(async move {
                        agent.session.lock().await.handle_publish(agent.options.peer_id(), publish);
                    });
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
                    if self.connected.replace(false) {
                        self.notify_connection(|l| l.on_disconnected());
                    }
                    if self.running.get() {
                        warn!("Messaging MQTT connection error: {error}");
                        debug!("Waiting 2s before reconnecting...");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
        debug!("MQTT event loop exited");
        self.ready.set(false);
        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
    }
}

impl MessagingClient for SessionAgent {
    fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

    fn peer_endpoint(&self) -> &str {
        self.options.peer_endpoint().as_str()
    }

    fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    async fn start(&self) -> Result<()> {
        let agent = self.self_reference.upgrade()
            .ok_or_else(|| StateError::new("Messaging session is no longer available"))?;
        let transport = self.session.lock().await.start_mqtt(&self.options).await?;
        if let Some((mqtt, eventloop)) = transport {
            self.running.set(true);
            let handle = task::spawn_local(agent.run_mqtt(mqtt, eventloop));
            *self.mqtt_task.borrow_mut() = Some(handle);
        }
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let result = self.session.lock().await.stop_mqtt().await;
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

    fn is_running(&self) -> bool {
        self.running.get()
    }

    fn is_connected(&self) -> bool {
        self.connected.get()
    }

    fn is_ready(&self) -> bool {
        self.ready.get()
    }

    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(SessionMessageBuilder {
            recipient,
            session: self.session.clone(),
            peer_id: *self.peer_id(),
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        })
    }

    async fn get_conversation(&self, id: &Id) -> Result<Option<Box<dyn Conversation>>> {
        self.session.lock().await.get_conversation(id).await
    }

    async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
        self.session.lock().await.get_conversations().await
    }

    async fn remove_conversation(&self, id: &Id) -> Result<()> {
        self.session.lock().await.remove_conversation(id).await
    }

    async fn remove_conversations(&self, ids: &[Id]) -> Result<()> {
        self.session.lock().await.remove_conversations(ids).await
    }

    async fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.session.lock().await.get_messages(conversation_id, until, limit, offset).await
    }

    async fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.session.lock().await.get_messages_in_range(conversation_id, begin, end).await
    }

    async fn remove_message(&self, message_id: i64) -> Result<()> {
        self.session.lock().await.remove_message(message_id).await
    }

    async fn remove_messages_by_ids(&self, message_ids: &[i64]) -> Result<()> {
        self.session.lock().await.remove_messages_by_ids(message_ids).await
    }

    async fn remove_messages_in_conversation(&self, conversation_id: &Id) -> Result<()> {
        self.session.lock().await.remove_messages_in_conversation(conversation_id).await
    }

    async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.session.lock().await.get_sessions().await
    }

    async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        self.session.lock().await.revoke_session(device_id).await
    }

    async fn friend_request(&self, user_id: &Id, hello: Option<String>) -> Result<()> {
        self.session.lock().await.friend_request(self.peer_id(), user_id, hello).await
    }

    async fn accept_friend_request(&self, user_id: &Id) -> Result<()> {
        self.session.lock().await.accept_friend_request(self.peer_id(), user_id).await
    }

    async fn get_friend_request(&self, user_id: &Id) -> Result<Option<FriendRequest>> {
        self.session.lock().await.get_friend_request(user_id).await
    }

    async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        self.session.lock().await.get_friend_requests().await
    }

    async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        self.session.lock().await.remove_friend_request(user_id).await
    }

    async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        self.session.lock().await.remove_friend_requests(user_ids).await
    }

    async fn clear_friend_requests(&self) -> Result<()> {
        self.session.lock().await.clear_friend_requests().await
    }

    async fn add_friend(
        &self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.session.lock().await.add_friend(user_id, session_key, remark).await
    }

    async fn block_user(&self, user_id: &Id) -> Result<Box<dyn Contact>> {
        self.session.lock().await.block_user(user_id).await
    }

    async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        self.session.lock().await.create_channel(permission, name, notice, announcement).await
    }

    async fn remove_channel(&self, channel_id: &Id) -> Result<()> {
        self.session.lock().await.remove_channel(channel_id).await
    }

    async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<dyn Channel>> {
        self.session.lock().await.join_channel(ticket).await
    }

    async fn leave_channel(&self, channel_id: &Id) -> Result<()> {
        self.session.lock().await.leave_channel(channel_id).await
    }

    async fn create_invite_ticket(&self, channel_id: &Id, invitee: Option<Id>) -> Result<InviteTicket> {
        self.session.lock().await.create_invite_ticket(channel_id, invitee).await
    }

    async fn transfer_channel_ownership(&self, channel_id: &Id, new_owner: Id) -> Result<()> {
        self.session.lock().await.transfer_channel_ownership(channel_id, new_owner).await
    }

    async fn rotate_channel_session_key(&self, channel_id: &Id) -> Result<()> {
        self.session.lock().await.rotate_channel_session_key(channel_id).await
    }

    async fn update_channel_info(&self, channel: &dyn Channel) -> Result<()> {
        self.session.lock().await.update_channel_info(channel).await
    }

    async fn set_channel_members_role(&self, channel_id: &Id, members: &[Id], role: Role) -> Result<()> {
        self.session.lock().await.set_channel_members_role(channel_id, members, role).await
    }

    async fn ban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.session.lock().await.ban_channel_members(channel_id, members).await
    }

    async fn unban_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.session.lock().await.unban_channel_members(channel_id, members).await
    }

    async fn remove_channel_members(&self, channel_id: &Id, members: &[Id]) -> Result<()> {
        self.session.lock().await.remove_channel_members(channel_id, members).await
    }

    async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        self.session.lock().await.get_contact(id).await
    }

    async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        self.session.lock().await.get_contacts().await
    }

    async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        self.session.lock().await.update_contact(contact).await
    }

    async fn remove_contact(&self, id: &Id) -> Result<()> {
        self.session.lock().await.remove_contact(id).await
    }

    async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        self.session.lock().await.remove_contacts(ids).await
    }

    async fn clear_contacts(&self) -> Result<()> {
        self.session.lock().await.clear_contacts().await
    }
}

struct SessionMessageBuilder {
    recipient: Option<Id>,
    session: Arc<tokio::sync::Mutex<Session>>,
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
            let (mqtt, user_identity, device_identity, friend_identity) = {
                let session = self.session.lock().await;
                let mqtt = session.mqtt.clone()
                    .ok_or_else(|| StateError::new("Messaging client is not running"))?;
                let friend_identity = session.friend_sessions.get(&recipient).cloned()
                    .ok_or_else(|| StateError::new(format!("No friend session for {recipient}")))?;
                (mqtt, session.user_identity.clone(), session.device_identity.clone(), friend_identity)
            };
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
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| StateError::new(format!("System clock error: {error}")))?
                .as_millis() as i64;
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
            let message_id = Session::message_id(device_identity.id(), timestamp)?;
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
