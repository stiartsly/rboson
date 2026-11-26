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

use rumqttc::{
    AsyncClient,
    QoS,
};
use serde_cbor::Value;
use boson_api::{
    Result,
    errors::{ArgumentError, StateError},
    CryptoIdentity,
    Id,
    Identity,
};

use crate::{
    errors::{
        AuthenticationError,
        EncodingError,
    },
    MessageType,
};
use crate::internal::{
    PhotonMessage,
    Topic,
};
use super::RpcServer;

use super::mqtt::MAX_MESSAGE_SIZE;

pub(super) fn seal_for(
    user_identity: &CryptoIdentity,
    device_identity: &CryptoIdentity,
    peer_id: &Id,
    recipient: &Id,
    message_type: i32,
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
    let created_at = i64::try_from(timestamp)
        .map_err(|_| EncodingError::new("Message timestamp is negative"))?;
    let message = PhotonMessage::outgoing(
        RpcServer::message_id(device_identity.id(), timestamp)?,
        *recipient,
        MessageType::from_i32(message_type).unwrap(),
        created_at,
        Value::Bytes(body),
    );
    let encoded = message.serialize();
    let payload = device_identity
        .encrypt_into(peer_id, &encoded)
        .map_err(|error| {
            AuthenticationError::new(format!("Encrypting {what} failed: {error}"))
        })?;
    Ok((message, payload))
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

