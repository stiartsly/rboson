use std::fmt;
use std::result::Result as SResult;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    Id,
    errors::{Error, Result},
    dht::msg::lookup_req::{
        LookupRequest,
        Data as LookupData,
        WANT4_MASK, WANT6_MASK,
    }
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
    #[serde(rename = "t")]
    target: Id,
    #[serde(rename = "w")]
    want: i32,
    #[serde(
        rename = "cas",
        skip_serializing_if = "is_default_expected_seq",
        default = "default_expected_seq",
        deserialize_with = "deserialize_expected_seq"
    )]
    expected_seq: i32,
    #[serde(
        rename = "e",
        skip_serializing_if = "crate::is_default",
        default,
        deserialize_with = "deserialize_expected_count"
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
        write!(
            f,
            "t:{},w:{}",
            self.target(),
            self.want()
        )?;
        if self.expected_seq >= 0 {
            write!(f, ",cas:{}", self.expected_seq)?;
        }
        write!(f, ",e:{}", self.expected_count)
    }
}

fn is_default_expected_seq(v: &i32) -> bool {
     *v < 0
}

fn default_expected_seq() -> i32 { -1 }

fn deserialize_expected_seq<'de, D>(de: D) -> SResult<i32, D::Error>
where  D: Deserializer<'de>,
{
    let seq = i32::deserialize(de)?;
    if seq < -1 {
        return Err(serde::de::Error::custom("expected_seq must be larger than or equal to -1"));
    }
    Ok(seq)
}

fn deserialize_expected_count<'de, D>(de: D) -> SResult<i32, D::Error>
where D: Deserializer<'de>,
{
    let count = i32::deserialize(de)?;
    if count <= 0 {
        return Err(serde::de::Error::custom("expected_count must be at least larger than 0"));
    }
    Ok(count)
}
