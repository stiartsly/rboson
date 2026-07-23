use std::fmt;
use serde::{Deserialize, Serialize};
use crate::{
    Id,
    errors::{Error, Result},
};
use super::{
    utils,
    lookup_req::{
        LookupRequest,
        Data as LookupData,
        WANT4_MASK, WANT6_MASK,
    }
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeFindValueRequest", try_from = "SerdeFindValueRequest")]
pub(crate) struct FindValueRequest {
    data: LookupData,
    expected_seq: i32,
}

impl FindValueRequest {
    pub(crate) fn new(
        target: Id,
        want4: bool, want6: bool,
        expected_seq: i32,
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, false),
            expected_seq,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }
}

impl LookupRequest for FindValueRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindValueRequest {
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
    expected_seq: i32
}

impl Into<SerdeFindValueRequest> for FindValueRequest {
    fn into(self) -> SerdeFindValueRequest {
        SerdeFindValueRequest {
            target: self.target().clone(),
            want: self.want(),
            expected_seq: self.expected_seq,
        }
    }
}

impl TryFrom<SerdeFindValueRequest> for FindValueRequest {
    type Error = Error;
    fn try_from(s: SerdeFindValueRequest) -> Result<Self> {
        Ok(FindValueRequest::new(
            s.target,
            s.want & WANT4_MASK != 0,
            s.want & WANT6_MASK != 0,
            s.expected_seq,
        ))
    }
}

impl fmt::Display for FindValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
