use std::fmt;
use serde::{Deserialize, Serialize};
use crate::{
    Id,
    errors::{Error, Result},
};
use crate::dht::{
    msg::utils,
    msg::lookup_req::{
        LookupRequest,
        Data as LookupData,
        WANT4_MASK, WANT6_MASK,
    },
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeFindPeerRequest", try_from = "SerdeFindPeerRequest")]
pub(crate) struct FindPeerRequest {
    data: LookupData,
    expected_seq: i32,
    expected_count: i32,
}

impl FindPeerRequest {
    pub(crate) fn new(
        target: Id, want4: bool,  want6: bool,
        expected_seq: i32,
        expected_count: i32,
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, false),
            expected_seq,
            expected_count,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn expected_count(&self) -> i32 {
        self.expected_count
    }
}

impl LookupRequest for FindPeerRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindPeerRequest {
    #[serde(
        rename = "t",
        serialize_with = "utils::serialize_id",
        deserialize_with = "utils::deserialize_id"
    )]
    target: Id,

    #[serde(rename = "w")]
    want: i32,

    #[serde(
        rename = "cas",
        skip_serializing_if = "utils::is_default_seq",
        default = "utils::default_seq",
        deserialize_with = "utils::deserialize_seq"
    )]
    expected_seq: i32,

    #[serde(
        rename = "e",
        skip_serializing_if = "crate::is_default",
        default,
        deserialize_with = "utils::deserialize_count"
    )]
    expected_count: i32,
}

impl Into<SerdeFindPeerRequest> for FindPeerRequest {
    fn into(self) -> SerdeFindPeerRequest {
        SerdeFindPeerRequest {
            target: self.target().clone(),
            want: self.want(),
            expected_seq: self.expected_seq,
            expected_count: self.expected_count,
        }
    }
}

impl TryFrom<SerdeFindPeerRequest> for FindPeerRequest {
    type Error = Error;
    fn try_from(s: SerdeFindPeerRequest) -> Result<Self> {
        Ok(FindPeerRequest::new(
            s.target,
            s.want & WANT4_MASK != 0,
            s.want & WANT6_MASK != 0,
            s.expected_seq,
            s.expected_count
        ))
    }
}

impl fmt::Display for FindPeerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
