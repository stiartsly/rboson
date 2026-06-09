use std::fmt;
use serde::{Serialize, Deserialize};
use crate::{
    Id, PeerInfo,
    errors::{Error, Result},
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeAnnouncePeerRequest", try_from = "SerdeAnnouncePeerRequest")]
pub(crate) struct AnnouncePeerRequest {
    token:  i32,
    peer:   PeerInfo,
    expected_seq: i32, // None means unset
}

impl AnnouncePeerRequest {
    pub(crate) fn new(
        peer: PeerInfo,
        token: i32,
        expected_seq: Option<i32>
    ) -> Self {
        Self {
            token,
            peer,
            expected_seq: expected_seq.unwrap_or(-1),
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn peer(&self) -> &PeerInfo {
        &self.peer
    }
}

#[derive(Serialize, Deserialize)]
struct SerdeAnnouncePeerRequest {
    #[serde(rename = "tok")]
    token: i32,
    #[serde(rename = "cas")]
    #[serde(skip_serializing_if = "utils::is_default_expected_seq")]
    #[serde(default = "utils::default_expected_seq")]
    #[serde(deserialize_with = "utils::deserialize_expected_seq")]
    expected_seq: i32,
    #[serde(rename = "k")]
    id: Id,
    #[serde(rename = "n")]
    nonce: Vec<u8>,
    #[serde(rename = "seq")]
    #[serde(skip_serializing_if = "crate::is_default")]
    #[serde(default = "utils::default_seq")]
    #[serde(deserialize_with = "utils::deserialize_seq")]
    seq: i32,
    #[serde(rename = "o")]
    #[serde(skip_serializing_if = "crate::is_default")]
    node_id: Option<Id>,
    #[serde(rename = "os")]
    #[serde(skip_serializing_if = "crate::is_default")]
    node_sig: Option<Vec<u8>>,
    #[serde(rename = "sig")]
    sig: Vec<u8>,
    #[serde(rename = "f")]
    fingerprint: u64,
    #[serde(rename = "e")]
    endpoint: String,
    #[serde(rename = "ex")]
    #[serde(skip_serializing_if = "crate::is_default")]
    extra: Option<Vec<u8>>,
}

impl Into<SerdeAnnouncePeerRequest> for AnnouncePeerRequest {
    fn into(self) -> SerdeAnnouncePeerRequest {
        let peer = self.peer;
        SerdeAnnouncePeerRequest {
            token   : self.token,
            expected_seq: self.expected_seq,
            id      : peer.id().clone(),
            nonce   : peer.nonce().to_vec(),
            seq     : peer.sequence_number(),
            node_id : if peer.is_authenticated() {
                peer.nodeid().cloned()
            } else {
                None
            },
            node_sig: if peer.is_authenticated() {
                peer.node_signature().map(|v| v.to_vec())
            } else {
                None
            },
            sig     : peer.signature().to_vec(),
            fingerprint: peer.fingerprint(),
            endpoint: peer.endpoint().to_string(),
            extra   : peer.extra_data().map(|v| v.to_vec()),
        }
    }
}

impl TryFrom<SerdeAnnouncePeerRequest> for AnnouncePeerRequest {
    type Error = Error;
    fn try_from(s: SerdeAnnouncePeerRequest) -> Result<Self> {
        let peer = PeerInfo::packed(
            s.id,
            s.nonce,
            s.seq,
            s.node_id,
            s.node_sig,
            s.sig,
            s.fingerprint,
            s.endpoint,
            s.extra
        );
        Ok(AnnouncePeerRequest {
            token: s.token,
            peer,
            expected_seq: s.expected_seq,
        })
    }
}

impl fmt::Display for AnnouncePeerRequest {
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
