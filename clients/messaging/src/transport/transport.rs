use serde::{
    de::DeserializeOwned,
    Serialize,
};
use serde_cbor::Value;
use rumqttc::{
    AsyncClient,
    QoS,
};

use boson_api::errors::{
    Result,
    ArgumentError,
    StateError,
};

use super::prelude::*;
use super::mqtt::MAX_MESSAGE_SIZE;

pub(super) fn seal_for(
    user_identity: &CryptoIdentity,
    device_identity: &CryptoIdentity,
    peer_id: &Id,
    recipient: &Id,
    message_type: u8,
    timestamp: i64,
    body: Vec<u8>,
    inner: Option<&Id>,
    what: &str,
) -> Result<(PhotonMessage, Vec<u8>)> {
    let body = match inner {
        Some(peer) => user_identity
            .encrypt_into(peer, &body)
            .map_err(|error| {
                AuthenticationError::new(format!("Encrypting {what} failed: {error}"))
            })?,
        None => body,
    };
    let created_at = u64::try_from(timestamp)
        .map_err(|_| EncodingError::new("Message timestamp is negative"))?;
    let message = PhotonMessage::new(
        MESSAGE_VERSION,
        RpcServer::message_id(device_identity.id(), timestamp)?,
        *recipient,
        message_type,
        None,
        created_at,
        Value::Bytes(body),
    );
    let encoded = to_cbor_bytes(&message, what)?;
    let payload = device_identity
        .encrypt_into(peer_id, &encoded)
        .map_err(|error| {
            AuthenticationError::new(format!("Encrypting {what} failed: {error}"))
        })?;
    Ok((message, payload))
}


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

/// Publishes an already sealed payload to the device outbox.
///
/// Used by the message builders, which hold the client handle but not the
/// [`Mqtt`] itself because they run on the caller's thread.
pub(crate) async fn publish_outbox(client: &AsyncClient, payload: Vec<u8>, what: &str) -> Result<()> {
    let payload_len = payload.len();
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

    client.publish(
        Topic::DeviceOutbox.as_str(),
        QoS::AtLeastOnce,
        false,
        payload
    )
    .await
    .map_err(|error| StateError::new(format!("Publishing {what} failed: {error}")).into())
}

