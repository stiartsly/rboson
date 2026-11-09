use tokio::time;

use super::*;
use super::mqtt::{
    Inbound,
    TransportEvent,
};

impl RpcServer {
    pub(crate) async fn poll_transport(&self) -> TransportEvent {
        if let Some(deadline) = self.reconnect_at.get() {
            time::sleep_until(deadline).await;
        }
        self.transport.poll().await
    }

    /// Dispatches one event from the transport.
    ///
    /// The transport has already reduced the raw `rumqttc` event to an
    /// [`Inbound`], or dropped it when the RPC server has no interest in it.
    pub(crate) fn handle_transport_event(
        self: &Rc<Self>,
        event: TransportEvent,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        let Some(inbound) = self.transport.interpret(event) else {
            return;
        };
        match inbound {
            Inbound::Connected {
                session_present,
                accepted,
            } => self.on_connack(accepted, session_present, pending),

            Inbound::Subscribed { accepted } => self.on_suback(accepted),

            Inbound::Message { topic, message } => {
                let session = self.clone();
                pending.push(
                    async move {
                        if let Err(error) = session.dispatch(topic, *message).await {
                            warn!("Failed to process MQTT message on '{}': {error}", &topic);
                        }
                    }
                    .boxed_local(),
                );
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
            &Topic::UserInbox,
            &Topic::UserOutbox,
            &Topic::DeviceInbox
        );

        let transport = self.transport.clone();
        let session = self.clone();
        pending.push(
            async move {
                if !transport.is_open() {
                    error!("MQTT transport disappeared before subscription");
                    session.close_transport("MQTT transport disappeared before subscription");
                    return;
                }
                if let Err(error) = transport.subscribe_topics().await {
                    error!("Failed to subscribe topics: {error}");
                    session.close_transport("MQTT topic subscription failed");
                }
            }
            .boxed_local(),
        );
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
        self.reconnect_at
            .set(Some(tokio::time::Instant::now() + Duration::from_secs(2)));
    }
}
