use tokio::time;
use log::trace;
use rumqttc::{
    Event,
    Incoming,
    Outgoing,
    Publish,
};

use super::prelude::*;
use super::mqtt::MAX_INCOMING_PACKET_SIZE;

impl RpcServer {
    pub(crate) async fn poll_event(&self) -> Result<Event> {
        if let Some(deadline) = self.reconnect_at.get() {
            time::sleep_until(deadline).await;
        }
        self.poll().await.map_err(|error| {
            boson_api::errors::StateError::new(format!("MQTT polling failed: {error}")).into()
        })
    }

    pub(crate) fn handle_poll_error(&self, error: &str) {
        self.on_transport_error(error);
    }

    pub(crate) fn dispatch_event(
        self: &Rc<Self>,
        event: Event,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        match event {
            Event::Incoming(Incoming::ConnAck(connack)) => {
                self.on_connack(
                    connack.code == rumqttc::ConnectReturnCode::Success,
                    connack.session_present,
                    pending,
                );
            }
            Event::Incoming(Incoming::SubAck(suback)) => {
                debug!(
                    "Received SubAck for packet {:?}, return codes: {:?}",
                    suback.pkid, suback.return_codes
                );
                let accepted = suback.return_codes.len() == 3
                    && suback.return_codes.iter()
                        .all(|code| matches!(code, rumqttc::SubscribeReasonCode::Success(_)));
                self.on_suback(accepted);
            }
            Event::Incoming(Incoming::Publish(publish)) => {
                self.on_publish(publish, pending);
            }
            Event::Incoming(Incoming::PubAck(puback)) => {
                trace!("Received PubAck for packet id {}", puback.pkid);
            }
            Event::Incoming(Incoming::PingResp) => {
                trace!("Received ping response from messaging server.");
            }
            Event::Incoming(Incoming::Disconnect) => {
                warn!("Messaging server sent DISCONNECT");
            }
            Event::Incoming(_) => {}
            Event::Outgoing(Outgoing::Publish(packet_id)) => {
                trace!("Sent outgoing MQTT publish packet id {}", packet_id);
            }
            Event::Outgoing(_) => {}
        }
    }

    fn on_publish(
        self: &Rc<Self>,
        publish: Publish,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        debug!(
            "Processing publish message on topic '{}', QoS: {:?}, payload size: {} bytes",
            publish.topic,
            publish.qos,
            publish.payload.len()
        );
        if publish.payload.len() > MAX_INCOMING_PACKET_SIZE {
            warn!(
                "Ignoring MQTT message on '{}': payload exceeds the incoming packet size limit",
                publish.topic
            );
            return;
        }
        let Ok(topic) = publish.topic.parse::<Topic>() else {
            warn!("Ignoring MQTT message on '{}': unexpected topic", publish.topic);
            return;
        };

        let payload = publish.payload.as_ref();
        let envelope = self.device_identity
            .decrypt_into(self.options.peer_id(), payload)
            .map_err(|e|
                AuthenticationError::new(format!("Decrypting packet failed: {e}"))
            );

        let envelope = match envelope {
            Ok(v) => v,
            Err(error) => {
                warn!("Ignoring MQTT message on '{}': {error}", publish.topic);
                return;
            }
        };

        let message = match decode_cbor_bytes(&envelope, "messaging envelope") {
            Ok(message) => message,
            Err(error) => {
                warn!("Ignoring MQTT message on '{}': {error}", publish.topic);
                return;
            }
        };

        let session = self.clone();
        let name = topic.as_str();
        pending.push(
            async move {
                if let Err(error) = session.dispatch(topic, message).await {
                    warn!("Failed to process MQTT message on '{}': {error}", name);
                }
            }
            .boxed_local(),
        );
    }

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
            &Topic::UserInbox,
            &Topic::UserOutbox,
            &Topic::DeviceInbox
        );

        let session = self.clone();
        pending.push(
            async move {
                if !session.is_open() {
                    error!("MQTT transport disappeared before subscription");
                    session.close_transport("MQTT transport disappeared before subscription");
                    return;
                }
                if let Err(error) = session.subscribe_topics().await {
                    error!("Failed to subscribe topics: {error}");
                    session.close_transport("MQTT topic subscription failed");
                }
            }
            .boxed_local(),
        );
    }

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

    fn on_transport_error(&self, error: &str) {
        self.ready.set(false);
        self.fail_pending_rpc(&format!("Messaging MQTT connection failed: {error}"));
        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
        if !self.running.get() {
            return;
        }

        match self.password() {
            Ok(password) => self.refresh_credentials(&password),
            Err(error) => {
                error!("Failed to refresh MQTT credentials: {error}");
                self.close_transport("Failed to refresh MQTT credentials");
                return;
            }
        }
        warn!("Messaging MQTT connection error: {error}");
        debug!("Waiting 2s before reconnecting...");
        self.reconnect_at
            .set(Some(tokio::time::Instant::now() + Duration::from_secs(2)));
    }

    fn open_envelope(&self, payload: &[u8]) -> Result<PhotonMessage> {
        let envelope = self.device_identity
            .decrypt_into(self.options.peer_id(), payload)
            .map_err(|e|
                AuthenticationError::new(format!("Decrypting packet failed: {e}"))
            )?;

        decode_cbor_bytes(&envelope, "messaging envelope")
    }
}
