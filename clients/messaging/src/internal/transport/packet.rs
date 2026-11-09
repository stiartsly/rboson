use boson_api::Result;
use rumqttc::{
    Outgoing,
    Incoming,
    Publish,
};

use super::*;

impl MqttClient {
    pub(crate) fn interpret(&self, event: TransportEvent) -> Option<Inbound> {
        let event = match event {
            Ok(v) => v,
            Err(error) => return Some(Inbound::Failed { error: error.to_string() }),
        };

        match event {
            Event::Incoming(packet) => self.on_incomming(packet),
            Event::Outgoing(packet) => self.on_outgoing(packet),
            _=> None,
        }
    }

    fn on_incomming(&self, packet: Incoming) -> Option<Inbound> {
        match packet {
            Incoming::ConnAck(connack) => Some(Inbound::Connected {
                session_present: connack.session_present,
                accepted: connack.code == rumqttc::ConnectReturnCode::Success,
            }),
            Incoming::SubAck(suback) => {
                debug!(
                    "Received SubAck for packet {:?}, return codes: {:?}",
                    suback.pkid, suback.return_codes
                );
                let accepted = suback.return_codes.len() == 3
                    && suback.return_codes.iter()
                        .all(|code| matches!(code, rumqttc::SubscribeReasonCode::Success(_)));
                Some(Inbound::Subscribed { accepted })
            }
            Incoming::Publish(publish) => Some(self.open_publish(publish)),
            Incoming::PubAck(puback) => {
                trace!("Received PubAck for packet id {}", puback.pkid);
                None
            }
            Incoming::PingResp => {
                trace!("Received PingResp from messaging server");
                None
            }
            _ => None,
        }
    }

    fn on_outgoing(&self, packet: Outgoing) -> Option<Inbound> {
        trace!("Sent outgoing MQTT packet: {:?}", packet);
        None
    }

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
        let Ok(topic) = publish.topic.parse::<Topic>() else {
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

    fn open(&self, payload: &[u8]) -> Result<PhotonMessage> {
        let envelope = self.device_identity
            .decrypt_into(self.options.peer_id(), payload)
            .map_err(|error| AuthenticationError::new(format!("Decrypting message envelope failed: {error}")))?;
        decode_cbor_bytes(&envelope, "messaging envelope")
    }
}
