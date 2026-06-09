use std::fmt;
use serde::{Deserialize, Serialize};
use crate::{
    Id,
    dht::msg::lookup_req::{
        LookupRequest,
        Data as LookupData,
        WANT4_MASK, WANT6_MASK, WANT_TOKEN_MASK,
    }
};

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeFindNodeRequest", from = "SerdeFindNodeRequest")]
pub(crate) struct FindNodeRequest {
    data: LookupData,
}

impl FindNodeRequest {
    pub(crate) fn new(target: Id,
        want4: bool,
        want6: bool,
        want_token: bool
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, want_token)
        }
    }
}

impl LookupRequest for FindNodeRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeFindNodeRequest {
    #[serde(rename = "t")]
    target: Id,
    #[serde(rename = "w")]
    want: i32,
}

impl Into<SerdeFindNodeRequest> for FindNodeRequest {
    fn into(self) -> SerdeFindNodeRequest {
        SerdeFindNodeRequest {
            target  : self.target().clone(),
            want    : self.want()
        }
    }
}

impl From<SerdeFindNodeRequest> for FindNodeRequest {
    fn from(s: SerdeFindNodeRequest) -> Self {
        FindNodeRequest::new(
            s.target,
            s.want & WANT4_MASK != 0,
            s.want & WANT6_MASK != 0,
            s.want & WANT_TOKEN_MASK != 0
        )
    }
}

impl fmt::Display for FindNodeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
