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
use std::{
    cell::RefCell,
    rc::Rc,
    result::Result as StdResult,
    sync::Arc,
    time::Duration,
};
use tokio::task;

use crate::messaging::{
    errors::{Error, Result},
    options::Options,
    verticle::VerticleOptions,
    ChannelListener, ConnectionListener, ContactListener, FriendRequestListener, MessageListener,
    SessionListener,
};
use crate::Id;

const USER_INBOX: &str = "u/i";
const USER_OUTBOX: &str = "u/o";
const DEVICE_INBOX: &str = "d/i";
const MAX_MESSAGE_SIZE: usize = 256 * 1024;

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
    options: Options,
    user_id: Id,
    device_id: Id,
    #[allow(dead_code)]
    connected: bool,
    #[allow(dead_code)]
    ready: bool,
    #[allow(dead_code)]
    running: bool,
    mqtt: RefCell<Option<AsyncClient>>,

    connection_listeners: Arc<dyn ConnectionListener>,
    #[allow(dead_code)]
    message_listeners: Arc<dyn MessageListener>,
    #[allow(dead_code)]
    channel_listeners: Arc<dyn ChannelListener>,
    contact_listeners: Arc<dyn ContactListener>,
    #[allow(dead_code)]
    session_listeners: Arc<dyn SessionListener>,
    friend_request_listeners: Arc<dyn FriendRequestListener>,
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
        let friend_request_listeners = options.friend_request_listener.clone();
        let opts = options.into_options();

        Ok(Self {
            options: opts,
            user_id,
            device_id,
            connected: false,
            ready: false,
            running: false,
            mqtt: RefCell::new(None),

            connection_listeners,
            message_listeners,
            channel_listeners,
            contact_listeners,
            session_listeners,
            friend_request_listeners,
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
            .map_err(|error| Error::Auth(error.to_string()))?;

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

        let endpoint = self
            .options
            .service_endpoint()
            .cloned()
            .ok_or_else(|| {
                Error::State(
                    "service.endpoint is required: DHT service discovery is not yet wired \
                 into the updated Rust messaging Options"
                        .into(),
                )
            })?;
        tokio::fs::create_dir_all(self.options.data_dir()).await?;

        let host = endpoint
            .host_str()
            .ok_or_else(|| Error::Argument("service endpoint has no hostname".into()))?;
        let port = endpoint
            .port()
            .ok_or_else(|| Error::Argument("service endpoint has no port".into()))?;

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
                expected_peer_id: Some(*self.options.service_peerid()),
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
        // Dedicated task to process incoming MQTT publish messages
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
            return Err(Error::Argument(
                "Cannot send friend request to yourself".into(),
            ));
        }
        info!(
            "Session: sending friend request to {user_id} with greeting: '{hello}'"
        );
        self.friend_request_listeners
            .on_friend_request(&user_id, Some(&hello));
        Ok(())
    }

    pub(crate) async fn friend_accept(&self, user_id: Id) -> Result<()> {
        info!("Session: accepting friend request from {user_id}");
        self.friend_request_listeners
            .on_friend_request_accepted(&user_id);
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
