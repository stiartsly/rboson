use std::fmt;
use serde::{Deserialize, Serialize};
use crate::{
    Id,
    Value,
    NodeInfo,
    cryptobox::Nonce,
    errors::{Error, Result, ProtocolError},
    dht::msg::lookup_rsp::{
        LookupResponse,
        Data
    }
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeFindValueResponse", try_from = "SerdeFindValueResponse")]
pub(crate) struct FindValueResponse {
    data: Data,
    value: Option<Value>,
}

impl FindValueResponse {
    pub(crate) fn with_nodes(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        Self {
            data: Data::new(nodes4, nodes6, 0),
            value: None,
        }
    }

    pub(crate) fn with_value(value: Value) -> Self {
        Self {
            data: Data::new(None, None, 0),
            value: Some(value),
        }
    }

    pub(crate) fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

impl LookupResponse for FindValueResponse {
    fn data(&self) -> &Data {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindValueResponse {
    #[serde(rename = "n4", skip_serializing_if = "crate::is_default")]
    nodes4: Option<Vec<NodeInfo>>,
    #[serde(rename = "n6", skip_serializing_if = "crate::is_default")]
    nodes6: Option<Vec<NodeInfo>>,
    #[serde(rename = "tok")]
    token: i32,
    #[serde(rename = "k", skip_serializing_if = "crate::is_default")]
    pk: Option<Id>,
    #[serde(rename = "rec", skip_serializing_if = "crate::is_default")]
    rec: Option<Id>,
    #[serde(rename = "n", skip_serializing_if = "crate::is_default")]
    nonce: Option<Vec<u8>>,
    #[serde(
        rename = "seq",
        skip_serializing_if = "utils::is_default_seq",
        default = "utils::default_seq",
        deserialize_with = "utils::deserialize_seq"
    )]
    expected_seq: i32,
    #[serde(rename = "sig", skip_serializing_if = "crate::is_default")]
    sig: Option<Vec<u8>>,
    #[serde(rename = "v", skip_serializing_if = "crate::is_default")]
    value: Option<Vec<u8>>,
}

impl Into<SerdeFindValueResponse> for FindValueResponse {
    fn into(self) -> SerdeFindValueResponse {
        SerdeFindValueResponse {
            nodes4  : self.nodes4().map(|v| v.to_vec()),
            nodes6  : self.nodes6().map(|v| v.to_vec()),
            token   : self.token(),
            pk      : self.value.as_ref().and_then(|v| v.public_key().cloned()),
            rec     : self.value.as_ref().and_then(|v| v.recipient().cloned()),
            nonce   : self.value.as_ref().and_then(|v| v.nonce().map(|n| n.as_ref().to_vec())),
            expected_seq: self.value.as_ref().map(|v| v.sequence_number()).unwrap_or(-1),
            sig     : self.value.as_ref().and_then(|v| v.signature().map(|s| s.to_vec())),
            value   : self.value.as_ref().map(|v| v.data().to_vec()),
        }
    }
}

impl TryFrom<SerdeFindValueResponse> for FindValueResponse {
    type Error = Error;
    fn try_from(s: SerdeFindValueResponse) -> Result<Self> {
        if s.value.is_none() &&
            s.nodes4.is_none() &&
            s.nodes6.is_none() {
            return Err(ProtocolError::new("either \"n4\", \"n6\" or \"v\" must be present"));
        }

        if s.value.is_some() && (
            s.nodes4.is_some() ||
            s.nodes6.is_some()
        ) {
            return Err(ProtocolError::new("\"v\" cannot be combined with \"n4\" or \"n6\""));
        }

        if let Some(data) = s.value {
            if data.is_empty() {
                return Err(ProtocolError::new("data field \"v\" cannot be empty"));
            }
            let expected_seq = s.expected_seq;
            if expected_seq < -1 {
                return Err(ProtocolError::new("sequence number must be larger than or equal to -1"));
            }
            let nonce = s.nonce.map(|v| {
                Nonce::try_from(v.as_slice())
                    .map_err(|_| ProtocolError::new("invalid nonce length"))
            }).transpose()?;

            let value = Value::packed(s.pk, s.rec, nonce, s.sig, data, expected_seq);
            if !value.is_valid() {
                return Err(ProtocolError::new("invalid value"));
            }

            Ok(Self::with_value(value))
        } else {
            Ok(Self::with_nodes(s.nodes4, s.nodes6))
        }
    }
}

impl fmt::Display for FindValueResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

mod utils {
    use serde::{Deserialize, de::Deserializer};
    use std::result::Result as SResult;

    pub(crate) fn is_default_seq(v: &i32) -> bool {
        *v < 0
    }

    pub(crate) fn default_seq() -> i32 { -1 }
    pub(crate) fn deserialize_seq<'de, D>(de: D) -> SResult<i32, D::Error>
    where  D: Deserializer<'de>,
    {
        let seq = i32::deserialize(de)?;
        if seq < -1 {
            return Err(serde::de::Error::custom("expected_seq must be larger than or equal to -1"));
        }
        Ok(seq)
    }
}
