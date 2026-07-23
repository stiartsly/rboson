use std::fmt;
use serde::{Deserialize, Serialize};
use crate::{
    NodeInfo,
    dht::msg::lookup_rsp::{
        LookupResponse,
        Data as LookupData
    }
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeFindNodeResponse", try_from = "SerdeFindNodeResponse")]
pub(crate) struct FindNodeResponse {
    data: LookupData,
}

impl FindNodeResponse {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>,
        token: i32
    ) -> Self {
        Self {
            data: LookupData::new(nodes4, nodes6, token)
        }
    }
}

impl LookupResponse for FindNodeResponse {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindNodeResponse {
    #[serde(rename = "n4", skip_serializing_if = "crate::is_default")]
    nodes4: Option<Vec<NodeInfo>>,
    #[serde(rename = "n6", skip_serializing_if = "crate::is_default")]
    nodes6: Option<Vec<NodeInfo>>,
    #[serde(rename = "tok")]
    token: i32,
}

impl Into<SerdeFindNodeResponse> for FindNodeResponse {
    fn into(self) -> SerdeFindNodeResponse {
        SerdeFindNodeResponse {
            nodes4: self.nodes4().map(|v| v.to_vec()),
            nodes6: self.nodes6().map(|v| v.to_vec()),
            token: self.token(),
        }
    }
}

impl From<SerdeFindNodeResponse> for FindNodeResponse {
    fn from(s: SerdeFindNodeResponse) -> Self {
        Self::new(
            s.nodes4,
            s.nodes6,
            s.token
        )
    }
}

impl fmt::Display for FindNodeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_value(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
