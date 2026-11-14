use tokio::time::{Instant, Duration};
use log::trace;
use rumqttc::{
    Event as MqttEvent,
    Incoming,
    Outgoing,
    Publish,
};
pub(crate) use futures::{
    stream::FuturesUnordered,
    FutureExt,
};

use boson_api::errors::StateError;

use super::prelude::*;
use super::mqtt::MAX_INCOMING_PACKET_SIZE;

impl RpcServer {
    pub(crate) async fn poll_event(&self) -> Result<MqttEvent> {
        if let Some(deadline) = self.reconnect_at.get() {
            tokio::time::sleep_until(deadline).await;
        }
        self.poll().await.map_err(|error| {
            StateError::new(format!("MQTT polling failed: {error}")).into()
        })
    }

    pub(crate) fn poll_event_error(&self, error: &str) {
        self.on_transport_error(error);
    }

    pub(crate) fn dispatch_event(
        self: &Rc<Self>,
        event: MqttEvent,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        match event {
            MqttEvent::Incoming(Incoming::ConnAck(connack)) => {
                self.on_connection_ack(connack, pending);
            }
            MqttEvent::Incoming(Incoming::SubAck(suback)) => {
                self.on_subscription_ack(suback);
            }
            MqttEvent::Incoming(Incoming::Publish(publish)) => {
                self.on_publish(publish, pending);
            }
            MqttEvent::Incoming(Incoming::PubAck(puback)) => {
                trace!("Received PubAck for packet id {}", puback.pkid);
            }
            MqttEvent::Incoming(Incoming::PingResp) => {
                trace!("Received ping response from messaging server.");
            }
            MqttEvent::Incoming(Incoming::Disconnect) => {
                warn!("Messaging server sent DISCONNECT");
                // TODO: Handle server-initiated disconnect if necessary
            }
            MqttEvent::Incoming(_) => {}
            MqttEvent::Outgoing(Outgoing::Publish(packet_id)) => {
                trace!("Sent outgoing MQTT publish packet id {}", packet_id);
            }
            MqttEvent::Outgoing(_) => {}
        }
    }

    fn on_connection_ack(
        self: &Rc<Self>,
        conack: rumqttc::ConnAck,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        debug!(
            "Received ConnAck from messaging server with code {:?} and session present {}",
            conack.code,
            conack.session_present
        );

        if conack.code != rumqttc::ConnectReturnCode::Success {
            warn!("Messaging MQTT connection was rejected with code {:?}", conack.code);
            return;
        }

        self.ready.set(false);
        info!(
            "Connected to messaging server (session_present: {})",
            conack.session_present
        );
        if !self.connected.replace(true) {
            self.connection_listener.as_ref().on_connected();
        }

        debug!(
            "Subscribing to topics: [{}, {}, {}]",
            &Topic::UserInbox,
            &Topic::UserOutbox,
            &Topic::DeviceInbox
        );

        let cloned_server = self.clone();
        pending.push(
            async move {
                if !cloned_server.is_open() {
                    error!("MQTT transport disappeared before subscription");
                    cloned_server.close_transport();
                    return;
                }
                if let Err(error) = cloned_server.subscribe_topics().await {
                    error!("Failed to subscribe topics: {error}");
                    cloned_server.close_transport();
                }
            }
            .boxed_local(),
        );
    }

    fn on_subscription_ack(
        self: &Rc<Self>,
        suback: rumqttc::SubAck
    ) {
        debug!(
            "Received SubAck for packet {:?}, return codes: {:?}",
            suback.pkid, suback.return_codes
        );

        let accepted = suback.return_codes.len() == 3
            && suback.return_codes.iter()
                .all(|code| matches!(code, rumqttc::SubscribeReasonCode::Success(_)));

        if !accepted {
            error!("Messaging MQTT topic subscription was rejected");
            self.close_transport();
            return;
        }

        if !self.connected.replace(true) {
            self.connection_listener.as_ref().on_connected();
        }
    }

    fn on_publish(
        self: &Rc<Self>,
        publish: rumqttc::Publish,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        debug!(
            "Received publish message on topic '{}', QoS: {:?}, payload size: {} bytes",
            publish.topic,
            publish.qos,
            publish.payload.len()
        );

        let (topic, message) = match self.try_on_publish(publish) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to decode publish: {}", e);
                return;
            }
        };

        let cloned_server = self.clone();
        pending.push(
            async move {
                cloned_server.dispatch(topic, message).await;
            }.boxed_local()
        );
    }

    fn try_on_publish(
        self: &Rc<Self>,
        publish: Publish,
    ) -> Result<(Topic, PhotonMessage)> {
        if publish.payload.len() > MAX_INCOMING_PACKET_SIZE {
            return Err(EncodingError::new(
                &format!("Ignored MQTT message on '{}': payload exceeds incoming packet size limit",
                 publish.topic
            )));
        }
        let topic = publish
            .topic
            .parse::<Topic>()
            .map_err(|_| {
                EncodingError::new(
                    &format!("Ignored MQTT message on '{}': unexpected topic", publish.topic)
                )
            })?;

        let payload = self
            .device_identity
            .decrypt_into(self.options.peer_id(), &publish.payload)
            .map_err(|_| {
                EncodingError::new(
                    &format!("Ignored MQTT message on '{}': decryption failed", publish.topic)
                )
            })?;

        let message: PhotonMessage = serde_cbor::from_slice(&payload)
            .map_err(|_| EncodingError::new(
                &format!("Ignored MQTT message on '{}': deserialization of Photon message failed",
                publish.topic
            )))?;

        if message.version() != MESSAGE_VERSION as i32 {
            return Err(EncodingError::new(
                &format!("Ignored MQTT message on '{}': unsupported message version", publish.topic)
            ));
        }

        Ok((topic, message))
    }

    fn on_transport_error(&self, error: &str) {
        self.ready.set(false);

        let reason = format!("Messaging MQTT connection failed: {error}");
        self.pending_calls.clear(&reason);

        if self.connected.replace(false) {
            self.connection_listener.as_ref().on_disconnected();
        }
        if !self.running.get() {
            return;
        }

        match self.password() {
            Ok(password) => self.refresh_credentials(&password),
            Err(error) => {
                error!("Failed to refresh MQTT credentials: {error}");
                self.close_transport();
                return;
            }
        }
        warn!("Messaging MQTT connection error: {error}");
        debug!("Waiting 2s before reconnecting...");
        self.reconnect_at
            .set(Some(Instant::now() + Duration::from_secs(2)));
    }
}
