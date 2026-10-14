use serde::{Deserialize, Serialize};

use crate::Id;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelSessionKeyRotation {
    #[serde(rename = "sid")]
    session_id: Id,
    #[serde(
        rename = "sk",
        serialize_with = "serialize_session_key",
        deserialize_with = "crate::utils::deserialize_bytes"
    )]
    session_key: Vec<u8>,
}

impl ChannelSessionKeyRotation {
    pub(crate) fn new(session_id: Id, session_key: Vec<u8>) -> Self {
        Self { session_id, session_key }
    }

    pub(crate) fn session_id(&self) -> &Id {
        &self.session_id
    }

    pub(crate) fn session_key(&self) -> &[u8] {
        &self.session_key
    }
}

fn serialize_session_key<S>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if serializer.is_human_readable() {
        crate::utils::serialize_bytes(value, serializer)
    } else {
        serializer.serialize_bytes(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn channel_session_rotation_round_trips_binary_key() {
        let dto = ChannelSessionKeyRotation::new(Id::default(), vec![1, 2, 3]);
        let value = serde_cbor::value::to_value(&dto).unwrap();
        let Value::Map(fields) = &value else {
            panic!("session rotation must serialize as a map");
        };

        assert_eq!(
            fields.get(&Value::Text("sk".to_string())),
            Some(&Value::Bytes(vec![1, 2, 3]))
        );
        let decoded: ChannelSessionKeyRotation =
            serde_cbor::from_slice(&serde_cbor::to_vec(&dto).unwrap()).unwrap();
        assert_eq!(decoded, dto);
    }
}
