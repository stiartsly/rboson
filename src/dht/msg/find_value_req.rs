use std::{
    fmt,
    result::Result as SResult
};
use serde::{
    Deserialize, Serialize,
    de::Deserializer
};

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
        write!(f, "t:{},w:{}", self.target(), self.want())?;
        if self.expected_seq >= 0 {
            write!(f, ",cas:{}", self.expected_seq)?;
        }
        Ok(())
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

