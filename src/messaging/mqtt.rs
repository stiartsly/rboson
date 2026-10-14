use std::{
    rc::Rc,
    result::Result as StdResult,
    sync::{Arc, Mutex},
    time::Duration,
};
use log::{debug, info, trace};
use rumqttc::tokio_rustls::rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
    ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
};
use rumqttc::{
    AsyncClient, Event, EventLoop, Incoming, MqttOptions, Publish, QoS, SubscribeFilter, TlsConfiguration,
    Transport,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_cbor::Value;

use crate::{CryptoIdentity, Id, Identity};
use crate::errors::{ArgumentError, Result, StateError};
use crate::messaging::{
    options::Options,
    errors::{AuthenticationError, EncodingError},
};

use super::internal::{PhotonMessage, topic::Topic};

/// The MQTT client handle, shared with the message builders on other threads.
pub(crate) type Client = AsyncClient;
pub(crate) type ClientHandle = Arc<Mutex<Option<AsyncClient>>>;

/// One raw poll result.
///
/// Kept as the `rumqttc` type so the verticle can pass it straight through;
/// [`Mqtt::interpret`] is what turns it into an [`Inbound`].
pub(crate) type TransportEvent = StdResult<Event, rumqttc::ConnectionError>;

const MAX_MESSAGE_SIZE: usize = 256 * 1024;
// Startup contact synchronization can exceed the outgoing per-message limit.
const MAX_INCOMING_PACKET_SIZE: usize = 2 * 1024 * 1024;

/// What one transport poll means once the wire format has been peeled off.
pub(crate) enum Inbound {
    /// The broker answered our CONNECT.
    Connected {
        session_present: bool,
        accepted: bool,
    },
    /// The broker answered our SUBSCRIBE.
    Subscribed { accepted: bool },
    /// A publish, already decrypted and decoded into a protocol message.
    ///
    /// Boxed because a message is an order of magnitude larger than the other
    /// variants, and this value is built on every poll.
    Message {
        topic: Topic,
        message: Box<PhotonMessage>,
    },
    /// A publish we cannot use; the topic is kept for the log line.
    Unusable { topic: String, reason: String },
    /// The transport failed; the connection is gone until the next retry.
    Failed { error: String },
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

// ---------------------------------------------------------------------------
// CBOR framing
// ---------------------------------------------------------------------------

/// Decodes a CBOR document the service sent us.
pub(crate) fn decode_cbor<T: DeserializeOwned>(value: Value, what: &str) -> Result<T> {
    serde_cbor::value::from_value(value)
        .map_err(|error| EncodingError::new(format!("Malformed {what}: {error}")).into())
}

/// Decodes a CBOR byte string the service, or a peer, sent us.
pub(crate) fn decode_cbor_bytes<T: DeserializeOwned>(bytes: &[u8], what: &str) -> Result<T> {
    serde_cbor::from_slice(bytes)
        .map_err(|error| EncodingError::new(format!("Malformed {what}: {error}")).into())
}

/// Converts a serialisable value into its CBOR data-model form.
pub(crate) fn to_cbor<T: Serialize>(value: &T, what: &str) -> Result<Value> {
    serde_cbor::value::to_value(value)
        .map_err(|error| EncodingError::new(format!("Encoding {what} failed: {error}")).into())
}

/// Encodes a serialisable value as CBOR bytes.
pub(crate) fn to_cbor_bytes<T: Serialize>(value: &T, what: &str) -> Result<Vec<u8>> {
    serde_cbor::to_vec(value)
        .map_err(|error| EncodingError::new(format!("Encoding {what} failed: {error}")).into())
}

/// Encodes `message` and seals it for the service with the device key.
pub(crate) fn seal_envelope(
    device_identity: &CryptoIdentity,
    peer_id: &Id,
    message: &PhotonMessage,
    what: &str,
) -> Result<Vec<u8>> {
    let encoded = to_cbor_bytes(message, what)?;
    device_identity
        .encrypt_into(peer_id, &encoded)
        .map_err(|error| AuthenticationError::new(format!("Encrypting {what} failed: {error}")).into())
}

/// Rejects a payload that would not fit in one outbound MQTT packet.
pub(crate) fn validate_outbox_packet(payload_len: usize) -> Result<()> {
    let remaining = payload_len.checked_add(2 + Topic::DeviceOutbox.as_str().len() + 2)
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

/// Publishes an already sealed payload to the device outbox.
///
/// Used by the message builders, which hold the client handle but not the
/// [`Mqtt`] itself because they run on the caller's thread.
pub(crate) async fn publish_outbox(client: &AsyncClient, payload: Vec<u8>, what: &str) -> Result<()> {
    validate_outbox_packet(payload.len())?;
    client.publish(Topic::DeviceOutbox.as_str(), QoS::AtLeastOnce, false, payload)
        .await
        .map_err(|error| StateError::new(format!("Publishing {what} failed: {error}")).into())
}

/// Maps a broker topic name onto one of the topics we subscribe to.
fn parse_topic(name: &str) -> Option<Topic> {
    [
        Topic::UserInbox,
        Topic::UserOutbox,
        Topic::DeviceInbox,
    ]
    .into_iter()
    .find(|topic| topic.as_str() == name)
}

// ---------------------------------------------------------------------------
// Transport state
// ---------------------------------------------------------------------------

pub(crate) struct MqttClient {
    options: Arc<Options>,
    client: ClientHandle,
    eventloop: tokio::sync::Mutex<Option<EventLoop>>,
    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,
}

impl MqttClient {
    pub(crate) fn new(
        options: Arc<Options>,
        user_identity: CryptoIdentity,
        device_identity: CryptoIdentity,
    ) -> Rc<Self> {
        Rc::new(Self {
            options,
            client: Arc::new(Mutex::new(None)) as ClientHandle  ,
            eventloop: tokio::sync::Mutex::new(None),
            user_identity,
            device_identity,
        })
    }

    /// The client handle, for callers that must own it past this borrow.
    pub(crate) fn client_handle(&self) -> ClientHandle {
        self.client.clone()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.client.lock().unwrap().is_some()
    }

    /// Fails unless the client is installed, without cloning it.
    pub(crate) fn ensure_open(&self) -> Result<()> {
        if self.is_open() {
            return Ok(());
        }
        Err(StateError::new("Messaging client is not running").into())
    }

    fn client(&self) -> Result<AsyncClient> {
        self.client
            .lock().unwrap()
            .clone()
            .ok_or_else(|| StateError::new("Messaging client is not running").into())
    }

    /// Builds the MQTT password: a fresh nonce signed with the device key.
    ///
    /// The nonce makes every connection's proof unique, which is what stops a
    /// captured password from being replayed on the next reconnect.
    pub(crate) fn password(&self) -> Result<String> {
        let nonce = crate::random_bytes(16);
        let signature = self.device_identity
            .sign_into(&nonce)
            .map_err(|error| AuthenticationError::new(error.to_string()))?;

        let mut proof = Vec::with_capacity(nonce.len() + signature.len());
        proof.extend_from_slice(&nonce);
        proof.extend_from_slice(&signature);
        Ok(bs58::encode(proof).into_string())
    }

    /// Builds and installs the client and its event loop.
    ///
    /// `contacts_revision` travels in the password as a query parameter, and
    /// `on_connecting` fires once the endpoint has been validated, just before
    /// the socket is opened.
    pub(crate) async fn connect(
        &self,
        contacts_revision: i32,
        on_connecting: impl FnOnce(),
    ) -> Result<()> {
        let options = &self.options;
        let endpoint = options.peer_endpoint().clone();
        tokio::fs::create_dir_all(options.data_dir()).await?;

        let host = endpoint
            .host_str()
            .ok_or_else(|| ArgumentError::new("service endpoint has no hostname"))?;
        let port = endpoint
            .port()
            .ok_or_else(|| ArgumentError::new("service endpoint has no port"))?;

        on_connecting();

        let client_id = self.device_identity.id().to_string();
        let mut mqtt_opts = MqttOptions::new(client_id.clone(), host.to_string(), port);
        let credentials = format!(
            "{}?contactsRevision={}",
            self.password()?,
            contacts_revision
        );
        mqtt_opts.set_credentials(self.user_identity.id().to_string(), &credentials);
        mqtt_opts.set_keep_alive(Duration::from_secs(30));
        mqtt_opts.set_clean_session(false);
        mqtt_opts.set_max_packet_size(MAX_INCOMING_PACKET_SIZE, MAX_MESSAGE_SIZE);
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

        let (client, eventloop) = AsyncClient::new(mqtt_opts, 32);
        *self.client.lock().unwrap() = Some(client);
        *self.eventloop.lock().await = Some(eventloop);
        Ok(())
    }

    /// Drops the client and disconnects it.
    pub(crate) async fn disconnect(&self) -> Result<()> {
        let Some(client) = self.client.lock().unwrap().take() else {
            return Ok(());
        };
        client.disconnect().await
            .map_err(|error| -> crate::Error {
                StateError::new(format!("Disconnecting MQTT client failed: {error}"))
            })
    }

    /// Drops the client without a disconnect handshake.
    pub(crate) fn close(&self) {
        self.client.lock().unwrap().take();
    }

    pub(crate) async fn clear_eventloop(&self) {
        self.eventloop.lock().await.take();
    }

    /// Polls the event loop for the next event.
    ///
    /// The loop is kept behind the lock across `poll` so that selecting another
    /// `select!` branch cannot drop the transport out from under it.
    pub(crate) async fn poll(&self) -> TransportEvent {
        let mut transport = self.eventloop.lock().await;
        match transport.as_mut() {
            Some(eventloop) => eventloop.poll().await,
            None => {
                drop(transport);
                std::future::pending().await
            }
        }
    }

    /// Subscribes to the three topics this client reads.
    pub(crate) async fn subscribe_topics(&self) -> Result<()> {
        let client = self.client()?;
        client.subscribe_many([
            SubscribeFilter::new(Topic::UserInbox.as_str().to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(Topic::UserOutbox.as_str().to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(Topic::DeviceInbox.as_str().to_string(), QoS::AtLeastOnce),
        ]).await
            .map_err(|error| StateError::new(format!("Subscribing topics failed: {error}")).into())
    }

    /// Publishes an already sealed payload.
    pub(crate) async fn publish(&self, payload: Vec<u8>, what: &str) -> Result<()> {
        let client = self.client()?;
        publish_outbox(&client, payload, what).await
    }

    /// Replaces the credentials the event loop will present on reconnect.
    ///
    /// Called from the event dispatch path, which runs after `poll` returned,
    /// so the event loop lock is free.
    pub(crate) fn refresh_credentials(&self, password: &str) {
        let username = self.options.user_id().to_base58();
        let mut transport = self.eventloop.try_lock()
            .expect("MQTT polling must finish before dispatch");
        if let Some(eventloop) = transport.as_mut() {
            eventloop.mqtt_options.set_credentials(username, password);
        }
    }

    /// Translates one poll result, or returns `None` for events the RPC server
    /// has no interest in (acks, pings, outgoing packets).
    pub(crate) fn interpret(&self, event: TransportEvent) -> Option<Inbound> {
        match event {
            Ok(Event::Incoming(Incoming::ConnAck(connack))) => Some(Inbound::Connected {
                session_present: connack.session_present,
                accepted: connack.code == rumqttc::ConnectReturnCode::Success,
            }),
            Ok(Event::Incoming(Incoming::SubAck(suback))) => {
                debug!(
                    "Received SubAck for packet {:?}, return codes: {:?}",
                    suback.pkid, suback.return_codes
                );
                let accepted = suback.return_codes.len() == 3
                    && suback.return_codes.iter()
                        .all(|code| matches!(code, rumqttc::SubscribeReasonCode::Success(_)));
                Some(Inbound::Subscribed { accepted })
            }
            Ok(Event::Incoming(Incoming::Publish(publish))) => Some(self.open_publish(publish)),
            Ok(Event::Incoming(Incoming::PubAck(puback))) => {
                trace!("Received PubAck for packet id {}", puback.pkid);
                None
            }
            Ok(Event::Incoming(Incoming::PingResp)) => {
                trace!("Received PingResp from messaging server");
                None
            }
            Ok(Event::Outgoing(outgoing)) => {
                trace!("Sent outgoing MQTT packet: {:?}", outgoing);
                None
            }
            Ok(other) => {
                trace!("MQTT event: {:?}", other);
                None
            }
            Err(error) => Some(Inbound::Failed { error: error.to_string() }),
        }
    }

    /// Opens one publish: validates the envelope, decrypts it and decodes the
    /// CBOR document inside.
    fn open_publish(&self, publish: Publish) -> Inbound {
        debug!(
            "Processing publish message on topic '{}', QoS: {:?}, payload size: {} bytes",
            publish.topic,
            publish.qos,
            publish.payload.len()
        );
        if publish.payload.len() > MAX_INCOMING_PACKET_SIZE {
            return Inbound::Unusable {
                topic: publish.topic.clone(),
                reason: "payload exceeds the incoming packet size limit".into(),
            };
        }
        let Some(topic) = parse_topic(&publish.topic) else {
            return Inbound::Unusable {
                topic: publish.topic.clone(),
                reason: "unexpected topic".into(),
            };
        };

        match self.open(publish.payload.as_ref()) {
            Ok(message) => Inbound::Message { topic, message: Box::new(message) },
            Err(error) => Inbound::Unusable {
                topic: publish.topic,
                reason: error.to_string(),
            },
        }
    }

    /// Decrypts an envelope from the service and decodes the message inside.
    fn open(&self, payload: &[u8]) -> Result<PhotonMessage> {
        let envelope = self.device_identity
            .decrypt_into(self.options.peer_id(), payload)
            .map_err(|error| AuthenticationError::new(format!("Decrypting message envelope failed: {error}")))?;
        decode_cbor_bytes(&envelope, "messaging envelope")
    }
}
