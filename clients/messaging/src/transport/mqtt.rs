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
    result::Result as StdResult,
    sync::{Arc, Mutex},
    time::Duration,
};
use log::{debug, info};
use rumqttc::{
    AsyncClient, Event, MqttOptions, QoS, SubscribeFilter, TlsConfiguration, Transport, tokio_rustls::rustls::ClientConfig
};

use boson_api::{
    Identity,
    Result,
    errors::StateError
};
use crate::{
    errors::{AuthenticationError},
};

use crate::internal::topic::Topic;
use crate::transport::RpcServer;

#[path = "cert_verifier.rs"]
mod cert_verifier;
use cert_verifier::CertVerifier;

pub(crate) type Client = AsyncClient;
pub(crate) type ClientHandle = Arc<Mutex<Option<AsyncClient>>>;

pub(super) const MAX_MESSAGE_SIZE: usize = 256 * 1024;
pub(super) const MAX_INCOMING_PACKET_SIZE: usize = 2 * 1024 * 1024;


// ---------------------------------------------------------------------------
// Transport state
// ---------------------------------------------------------------------------

impl RpcServer {
    pub(crate) fn client_handle(&self) -> ClientHandle {
        self.mqtt_client.clone()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.mqtt_client.lock().unwrap().is_some()
    }

    /*/// Fails unless the client is installed, without cloning it.
    pub(crate) fn ensure_open(&self) -> Result<()> {
        if self.is_open() {
            return Ok(());
        }
        Err(StateError::new("Messaging client is not running").into())
    }*/

    fn client(&self) -> Result<AsyncClient> {
        self.mqtt_client
            .lock().unwrap()
            .clone()
            .ok_or_else(|| StateError::new("Messaging client is not running").into())
    }

    pub(super) fn password(&self) -> boson_api::Result<String> {
        let nonce = boson_api::random_bytes(16);
        let signature = self.device_identity
            .sign_into(&nonce)
            .map_err(|error| AuthenticationError::new(error.to_string()))?;

        let mut proof = Vec::with_capacity(nonce.len() + signature.len());
        proof.extend_from_slice(&nonce);
        proof.extend_from_slice(&signature);

        Ok(bs58::encode(proof).into_string())
    }

    pub(crate) async fn connect(
        &self,
        contacts_revision: i32,
        on_connecting: impl FnOnce(),
    ) -> Result<()> {
        let options = &self.options;

       // tokio::fs::create_dir_all(options.data_dir()).await?;

        let endpoint = self.options.peer_endpoint().clone();
        let host = endpoint.host_str().unwrap().to_string();
        let port = endpoint.port().unwrap();

        on_connecting();

        let client_id = self.device_identity.id().to_string();
        let user_id   = self.user_identity.id().to_string();

        let mut opts = MqttOptions::new(client_id.clone(), host, port);
        let credentials = format!(
            "{}?contactsRevision={}",
            self.password()?,
            contacts_revision
        );
        opts.set_credentials(&user_id, &credentials);
        opts.set_keep_alive(Duration::from_secs(30));
        opts.set_clean_session(false);
        opts.set_max_packet_size(MAX_INCOMING_PACKET_SIZE, MAX_MESSAGE_SIZE);

        let using_tls = endpoint.scheme() == "mqtts" ||
                        endpoint.scheme() == "ssl" ||
                        port == 9083;
        if using_tls {
            let verifier = CertVerifier::new(Some(*options.peer_id()));
            let client_config = ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(verifier))
                .with_no_client_auth();

            let tls_config = TlsConfiguration::Rustls(Arc::new(client_config));
            opts.set_transport(Transport::tls_with_config(tls_config));
        }

        info!(
            "Connecting to messaging server at {} (TLS: {})",
            endpoint, using_tls
        );
        debug!(
            "MQTT client options: clientId={}, username={}, keepAlive=30s, cleanSession=false",
            client_id,
            self.user_identity.id()
        );

        let (client, eventloop) = AsyncClient::new(opts, 32);
        *self.mqtt_client.lock().unwrap() = Some(client);
        *self.mqtt_eventloop.lock().await = Some(eventloop);
        Ok(())
    }

    pub(crate) async fn disconnect(&self) -> Result<()> {
        let Some(client) = self.mqtt_client.lock().unwrap().take() else {
            return Ok(());
        };

        client.disconnect().await
            .map_err(|error| -> boson_api::Error {
                StateError::new(format!("Disconnecting from messaging server error: {error}"))
            })
    }

    /// Drops the client without a disconnect handshake.
    pub(crate) fn close(&self) {
        self.mqtt_client.lock().unwrap().take();
    }

    pub(crate) fn clear_eventloop(&self) {
        if let Ok(mut eventloop) = self.mqtt_eventloop.try_lock() {
            eventloop.take();
        }
    }

    /// Polls the event loop for the next event.
    ///
    /// The loop is kept behind the lock across `poll` so that selecting another
    /// `select!` branch cannot drop the transport out from under it.
    pub(crate) async fn poll(&self) -> StdResult<Event, rumqttc::ConnectionError> {
        let mut transport = self.mqtt_eventloop.lock().await;
        match transport.as_mut() {
            Some(eventloop) => eventloop.poll().await,
            None => {
                drop(transport);
                std::future::pending().await
            }
        }
    }

    pub(crate) async fn subscribe_topics(&self) -> Result<()> {
        let client = self.client()?;
        let topics = [
            SubscribeFilter::new(Topic::UserInbox.to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(Topic::UserOutbox.to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(Topic::DeviceInbox.to_string(), QoS::AtLeastOnce),
        ];
        client.subscribe_many(topics).await.map_err(|e|
            StateError::new(format!("Subscribing topics failed: {e}")).into()
        )
    }

    pub(crate) async fn publish(&self, payload: Vec<u8>, what: &str) -> Result<()> {
        let client = self.client()?;
        super::transport::publish_outbox(&client, payload, what).await
    }

    pub(crate) fn refresh_credentials(&self, password: &str) {
        let username = self.options.user_id().to_base58();
        let mut transport = self.mqtt_eventloop.try_lock()
            .expect("MQTT polling must finish before dispatch");
        if let Some(eventloop) = transport.as_mut() {
            eventloop.mqtt_options.set_credentials(username, password);
        }
    }

}

