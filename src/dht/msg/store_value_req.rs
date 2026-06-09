use std::fmt;
use serde::{Serialize, Deserialize};
use crate::{
    Id,
    Value,
    cryptobox::Nonce,
    errors::{Error, Result, ProtocolError},
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeStoreValueRequest", try_from = "SerdeStoreValueRequest")]
pub(crate) struct StoreValueRequest {
    token: i32,
    expected_seq: i32,
    value: Value,
}

impl StoreValueRequest {
    pub(crate) fn new(
        value: Value, token: i32, expected_seq: i32
    ) -> Self {
        Self { token, expected_seq, value }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Serialize, Deserialize)]
struct SerdeStoreValueRequest {
    #[serde(rename = "tok")]
    token: i32,
    #[serde(rename = "cas")]
    #[serde(skip_serializing_if = "utils::is_default_expected_seq")]
    #[serde(default = "utils::default_expected_seq")]
    #[serde(deserialize_with = "utils::deserialize_expected_seq")]
    expected_seq: i32,
    #[serde(rename = "seq")]
    #[serde(default = "utils::default_seq")]
    #[serde(skip_serializing_if = "crate::is_default")]
    #[serde(deserialize_with = "utils::deserialize_seq")]
    seq: i32,
    #[serde(rename = "k", skip_serializing_if = "crate::is_default")]
    public_key: Option<Id>,
    #[serde(rename = "rec", skip_serializing_if = "crate::is_default")]
    recipient: Option<Id>,
    #[serde(rename = "n", skip_serializing_if = "crate::is_default")]
    nonce: Option<Vec<u8>>,
    #[serde(rename = "sig", skip_serializing_if = "crate::is_default")]
    signature: Option<Vec<u8>>,
    #[serde(rename = "v")]
    data: Vec<u8>,
}

impl Into<SerdeStoreValueRequest> for StoreValueRequest {
    fn into(self) -> SerdeStoreValueRequest {
        let value = self.value;
        SerdeStoreValueRequest {
            token       : self.token,
            expected_seq: self.expected_seq,
            seq         : value.sequence_number(),
            public_key  : value.public_key().cloned(),
            recipient   : value.recipient().cloned(),
            nonce       : value.nonce().map(|n| n.as_bytes().to_vec()),
            signature   : value.signature().map(|v| v.to_vec()),
            data        : value.data().to_vec(),
        }
    }
}

impl TryFrom<SerdeStoreValueRequest> for StoreValueRequest {
    type Error = Error;

    fn try_from(s: SerdeStoreValueRequest) -> Result<Self> {
        if s.data.is_empty() {
            return Err(ProtocolError::new("data field \"v\" cannot be empty"));
        }

        let nonce = s.nonce.map(|v| {
            Nonce::try_from(v.as_slice())
                .map_err(|_| ProtocolError::new("invalid nonce length"))
        }).transpose()?;

        let value = Value::packed(
            s.public_key,
            s.recipient,
            nonce,
            s.signature,
            s.data,
            s.seq
        );
        // if !value.is_valid() {
        //     return Err(ProtocolError::new("The value is invalid"));
        // }

        Ok(StoreValueRequest::new(
            value,
            s.token,
            s.expected_seq
        ))
    }
}

impl fmt::Display for StoreValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

mod utils {
    use serde::{Deserialize, Deserializer};
    use std::result::Result as SResult;

    pub(crate) fn is_default_expected_seq(v: &i32) -> bool {
        *v < 0
    }

    pub(crate) fn default_expected_seq() -> i32 { -1 }
    pub(crate) fn deserialize_expected_seq<'de, D>(de: D) -> SResult<i32, D::Error>
    where  D: Deserializer<'de>,
    {
        let seq = i32::deserialize(de)?;
        if seq < -1 {
            return Err(serde::de::Error::custom("expected_seq must be larger than or equal to -1"));
        }
        Ok(seq)
    }

    pub(crate) fn default_seq() -> i32 { 0 }
    pub(crate) fn deserialize_seq<'de, D>(de: D) -> SResult<i32, D::Error>
    where  D: Deserializer<'de>,
    {
        let seq = i32::deserialize(de)?;
        if seq < 0 {
            return Err(serde::de::Error::custom("seq must be non-negative"));
        }
        Ok(seq)
    }
}
