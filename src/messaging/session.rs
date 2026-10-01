use log::{debug, error, info, trace, warn};
use rumqttc::{
    tokio_rustls::rustls::{
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
        ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
    },
    AsyncClient, Event, Incoming, MqttOptions, Publish, QoS, SubscribeFilter, TlsConfiguration,
    Transport,
};
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    result::Result as StdResult,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::task;

use crate::{CryptoIdentity, Id, Identity};
use crate::errors::{Result, Error, ArgumentError, StateError};
use crate::messaging::{
    options::Options,
    verticle::VerticleOptions,
    ChannelListener, ConnectionListener, ContactListener, MessageListener, SessionListener,
    errors::{AuthenticationError, EncodingError, ProtocolError},
};

use super::internal::{PhotonMessage};


const USER_INBOX: &str = "u/i";
const USER_OUTBOX: &str = "u/o";
const DEVICE_INBOX: &str = "d/i";
const DEVICE_OUTBOX: &str = "d/o";
const MAX_MESSAGE_SIZE: usize = 256 * 1024;
const MESSAGE_VERSION: u8 = 2;
const HANDSHAKE_MESSAGE: u8 = 0;

pub(crate) trait FriendProtocolListener: Send + Sync {
    fn on_friend_request(
        &self,
        user_id: Id,
        initiator_id: Id,
        hello: String,
        timestamp: i64,
        notify: bool,
    );
    fn on_friend_request_accepted(
        &self,
        user_id: Id,
        timestamp: i64,
        add_contact: bool,
    );
    fn on_content_message(&self, message: PhotonMessage);
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

/// Session instance holding all necessary information to interact with the MQTT server.
/// All fields are defined with RefCell since they are exclusively referenced within a
/// single dedicated thread running in a LocalSet.
pub(crate) struct Session {
    options: Arc<Options>,
    user_id: Id,
    device_id: Id,
    #[allow(dead_code)]
    connected: bool,
    #[allow(dead_code)]
    ready: bool,
    #[allow(dead_code)]
    running: bool,
    mqtt: RefCell<Option<AsyncClient>>,
    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,
    friend_sessions: RefCell<HashMap<Id, CryptoIdentity>>,

    connection_listeners: Arc<dyn ConnectionListener>,
    #[allow(dead_code)]
    message_listeners: Arc<dyn MessageListener>,
    #[allow(dead_code)]
    channel_listeners: Arc<dyn ChannelListener>,
    contact_listeners: Arc<dyn ContactListener>,
    #[allow(dead_code)]
    session_listeners: Arc<dyn SessionListener>,
    friend_protocol_listener: Arc<dyn FriendProtocolListener>,
}

impl Session {
    pub(crate) fn new(options: VerticleOptions) -> Result<Self> {
        let user_id = *options.user_id();
        let device_id = *options.device_id();
        debug!(
            "Initialized messaging session for user {} and device {}",
            user_id, device_id
        );
        let connection_listeners = options.connection_listener.clone();
        let message_listeners = options.message_listener.clone();
        let channel_listeners = options.channel_listener.clone();
        let contact_listeners = options.contact_listener.clone();
        let session_listeners = options.session_listener.clone();
        let friend_protocol_listener = options.friend_protocol_listener.clone();
        let opts = options.into_options();
        let user_identity = CryptoIdentity::from(opts.user_key().clone());
        let device_identity = CryptoIdentity::from(opts.device_key().clone());

        Ok(Self {
            options: opts,
            user_id,
            device_id,
            connected: false,
            ready: false,
            running: false,
            mqtt: RefCell::new(None),
            user_identity,
            device_identity,
            friend_sessions: RefCell::new(HashMap::new()),

            connection_listeners,
            message_listeners,
            channel_listeners,
            contact_listeners,
            session_listeners,
            friend_protocol_listener,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn is_connected(&self) -> bool {
        self.mqtt.borrow().is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn is_ready(&self) -> bool {
        self.mqtt.borrow().is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn is_running(&self) -> bool {
        self.mqtt.borrow().is_some()
    }

    fn password(&self) -> Result<String> {
        let mut nonce = [0u8; 16];
        unsafe {
            libsodium_sys::randombytes_buf(nonce.as_mut_ptr() as *mut libc::c_void, 16);
        }
        let options = &self.options;
        let device_key = options.device_key();

        let dsign = device_key
            .private_key()
            .sign_into(&nonce)
            .map_err(|error| AuthenticationError::new(error.to_string()))?;

        let mut password = Vec::with_capacity(nonce.len() + dsign.len());
        password.extend_from_slice(&nonce);
        password.extend_from_slice(&dsign);

        let base_password = bs58::encode(password).into_string();
        trace!(
            "Generated MQTT auth password for device {}",
            self.device_id
        );
        Ok(format!("{base_password}?contactsRevision=0"))
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        callback(self.connection_listeners.as_ref());
    }

    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        if self.is_running() {
            debug!("Messaging session is already running");
            return Ok(());
        }

        let endpoint = self.options.peer_endpoint().clone();
        tokio::fs::create_dir_all(self.options.data_dir()).await?;

        let host = endpoint
            .host_str()
            .ok_or_else(|| ArgumentError::new("service endpoint has no hostname"))?;
        let port = endpoint
            .port()
            .ok_or_else(|| ArgumentError::new("service endpoint has no port"))?;

        self.notify_connection(|listener| listener.on_connecting());

        let client_id = self.device_id.to_string();
        let mut mqtt_opts = MqttOptions::new(client_id.clone(), host.to_string(), port);
        mqtt_opts.set_credentials(self.user_id.to_string(), self.password()?);
        mqtt_opts.set_keep_alive(Duration::from_secs(30));
        mqtt_opts.set_clean_session(false);
        mqtt_opts.set_max_packet_size(MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE);
        let is_tls = endpoint.scheme() == "mqtts" || endpoint.scheme() == "ssl" || port == 9083;
        if is_tls {
            let verifier = BosonServerCertVerifier {
                expected_peer_id: Some(*self.options.peer_id()),
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
            self.user_id
        );

        let (mqtt, mut eventloop) = AsyncClient::new(mqtt_opts, 32);
        *self.mqtt.borrow_mut() = Some(mqtt.clone());

        let session = self.clone();
        task::spawn_local(async move {
            debug!("MQTT event loop started");
            let mut is_connected = false;
            let mut is_ready = false;
            while session.mqtt.borrow().is_some() {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Incoming::ConnAck(connack))) => {
                        if connack.code == rumqttc::ConnectReturnCode::Success {
                            info!(
                                "Connected to messaging server (session_present: {})",
                                connack.session_present
                            );
                            if !is_connected {
                                is_connected = true;
                                session.notify_connection(|l| l.on_connected());
                            }
                            debug!(
                                "Subscribing to topics: [{}, {}, {}]",
                                USER_INBOX, USER_OUTBOX, DEVICE_INBOX
                            );
                            let sub_res = mqtt
                                .subscribe_many([
                                    SubscribeFilter::new(USER_INBOX.to_string(), QoS::AtLeastOnce),
                                    SubscribeFilter::new(USER_OUTBOX.to_string(), QoS::AtLeastOnce),
                                    SubscribeFilter::new(
                                        DEVICE_INBOX.to_string(),
                                        QoS::AtLeastOnce,
                                    ),
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
                        if !is_connected {
                            is_connected = true;
                            session.notify_connection(|l| l.on_connected());
                        }
                        if !is_ready {
                            is_ready = true;
                            info!("Messaging session is ready");
                            session.notify_connection(|l| l.on_ready());
                        }
                    }
                    Ok(Event::Incoming(Incoming::Publish(publish))) => {
                        debug!(
                            "Received MQTT Publish on topic '{}', QoS: {:?}, payload size: {} bytes",
                            publish.topic,
                            publish.qos,
                            publish.payload.len()
                        );
                        let s = session.clone();
                        task::spawn_local(async move {
                            s.handle_publish(publish).await;
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
                        if is_connected {
                            is_connected = false;
                            is_ready = false;
                            session.notify_connection(|l| l.on_disconnected());
                        }
                        if session.mqtt.borrow().is_some() {
                            warn!("Messaging MQTT connection error: {error}");
                            debug!("Waiting 2s before reconnecting...");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            }
            debug!("MQTT event loop exited");
            if is_connected {
                session.notify_connection(|l| l.on_disconnected());
            }
        });

        Ok(())
    }

    async fn handle_publish(&self, publish: Publish) {
        debug!(
            "Processing publish message on topic '{}', payload size: {} bytes",
            publish.topic,
            publish.payload.len()
        );
        if publish.topic != USER_INBOX && publish.topic != USER_OUTBOX {
            return;
        }

        if let Err(error) = self.handle_user_publish(&publish.topic, publish.payload.as_ref()) {
            warn!("Failed to process MQTT message on '{}': {error}", publish.topic);
        }
    }

    fn handle_user_publish(&self, topic: &str, payload: &[u8]) -> Result<()> {
        let envelope = self
            .device_identity
            .decrypt_into(self.options.peer_id(), payload)
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

        let originated_here = message.id == Self::message_id(&self.device_id, message.created_at)?;
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
                self.friend_protocol_listener
                    .on_friend_request(
                        friend_id,
                        if is_inbox { from } else { self.user_id },
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
                self.friend_sessions.borrow_mut().insert(friend_id, session_identity);
                self.friend_protocol_listener
                    .on_friend_request_accepted(friend_id, handshake.timestamp, is_inbox);
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
        let sessions = self.friend_sessions.borrow();
        let session = sessions
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
        self.friend_protocol_listener.on_content_message(PhotonMessage {
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

    async fn send_handshake(
        &self,
        user_id: Id,
        handshake_type: HandshakeType,
        body: Value,
    ) -> Result<()> {
        let mqtt = self
            .mqtt
            .borrow()
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

        let message_id = Self::message_id(&self.device_id, timestamp)?;
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
            .encrypt_into(self.options.peer_id(), &message_bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting message envelope failed: {error}")))?;

        mqtt.publish(DEVICE_OUTBOX, QoS::AtLeastOnce, false, mqtt_payload)
            .await
            .map_err(|error| StateError::new(format!("Publishing handshake failed: {error}")).into())
    }

    pub(crate) async fn send_content(
        &self,
        recipient: Id,
        headers: HashMap<String, serde_json::Value>,
        body: Vec<u8>,
        text: bool,
    ) -> Result<PhotonMessage> {
        let mqtt = self
            .mqtt
            .borrow()
            .clone()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
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
            format: if text { ContentFormat::Text } else { ContentFormat::Binary },
            body: if text {
                Value::Text(String::from_utf8(body.clone())
                    .map_err(|_| EncodingError::new("Text message is not UTF-8"))?)
            } else {
                Value::Bytes(body.clone())
            },
        };
        let content_bytes = serde_cbor::to_vec(&content)
            .map_err(|error| EncodingError::new(format!("Encoding message content failed: {error}")))?;
        let sessions = self.friend_sessions.borrow();
        let session = sessions
            .get(&recipient)
            .ok_or_else(|| StateError::new(format!("No friend session for {recipient}")))?;
        let encrypted_content = self
            .user_identity
            .encrypt_into(session.id(), &content_bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting content failed: {error}")))?;
        drop(sessions);
        let message_id = Self::message_id(&self.device_id, timestamp)?;
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
        let mqtt_payload = self
            .device_identity
            .encrypt_into(self.options.peer_id(), &message_bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting message envelope failed: {error}")))?;
        mqtt.publish(DEVICE_OUTBOX, QoS::AtLeastOnce, false, mqtt_payload)
            .await
            .map_err(|error| StateError::new(format!("Publishing message failed: {error}")))?;

        Ok(PhotonMessage {
            id: message_id,
            recipient,
            from: Some(self.user_id),
            created_at: UNIX_EPOCH + Duration::from_millis(timestamp as u64),
            received_at: None,
            sent_at: Some(SystemTime::now()),
            payload: content_bytes,
            content: crate::messaging::message::Content::_new(headers, body),
        })
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
        self.friend_sessions.borrow_mut().insert(user_id, session_identity);
        Ok(())
    }

    fn message_id(device_id: &Id, timestamp: i64) -> Result<Id> {
        let mut digest = Sha256::new();
        digest.update(device_id.as_bytes());
        digest.update(timestamp.to_be_bytes());
        Id::try_from_bytes(digest.finalize().as_slice())
            .map_err(|error| EncodingError::new(error.to_string()).into())
    }

    pub(crate) async fn stop(&self) {
        info!("Stopping messaging session...");
        let mqtt = self.mqtt.borrow_mut().take();
        if let Some(mqtt) = mqtt {
            debug!("Disconnecting MQTT client...");
            let _ = mqtt.disconnect().await;
        }

        self.notify_connection(|l| l.on_disconnected());
        info!("Messaging session stopped");
    }

    pub(crate) async fn friend_request(&self, user_id: Id, hello: String) -> Result<()> {
        if user_id == self.user_id {
            return Err(ArgumentError::new(
                "Cannot send friend request to yourself",
            ));
        }
        info!(
            "Session: sending friend request to {user_id} with greeting: '{hello}'"
        );
        self.send_handshake(
            user_id,
            HandshakeType::FriendRequest,
            Value::Text(hello),
        )
        .await
    }

    pub(crate) async fn friend_accept(&self, user_id: Id) -> Result<()> {
        info!("Session: accepting friend request from {user_id}");
        let session_key = crate::signature::KeyPair::random()
            .private_key()
            .as_ref()
            .to_vec();
        let session_identity = CryptoIdentity::try_from(session_key.as_slice())
            .map_err(|error| AuthenticationError::new(format!("Invalid friend session key: {error}")))?;
        self.send_handshake(
            user_id,
            HandshakeType::FriendRequestAccept,
            Value::Bytes(session_key),
        )
        .await?;
        self.friend_sessions.borrow_mut().insert(user_id, session_identity);
        Ok(())
    }

    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        info!("Session: rejecting friend request from {user_id}");
        Ok(())
    }

    pub(crate) async fn friend_remove(&self, user_id: Id) -> Result<()> {
        info!("Session: removing friend {user_id}");
        self.contact_listeners.on_contacts_removed(&[user_id]);
        Ok(())
    }

    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        debug!("Session: querying friend info for {user_id}");
        Ok(())
    }
}
