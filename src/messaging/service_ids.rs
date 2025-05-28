use serde::Deserialize;

use crate::{
    Id,
    Error,
    error::Result,
};

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub(crate) struct JsonServiceIds {
    peerId: String,
    nodeId: String
}

impl JsonServiceIds {
    pub(crate) fn ids(&self) -> Result<ServiceIds> {
        let Ok(peerid) = Id::try_from(self.peerId.as_str()) else {
            return Err(Error::State("Http error: invalid peer id".into()));
        };
        let Ok(nodeid) = Id::try_from(self.nodeId.as_str()) else {
            return Err(Error::State("Http error: invalid node id".into()));
        };

        Ok(ServiceIds {
            peerid,
            nodeid
        })
    }
}

#[derive(Debug, Clone)]
pub struct ServiceIds {
    peerid: Id,
    nodeid: Id
}

impl ServiceIds {
    pub fn peerid(&self) -> &Id {
        &self.peerid
    }

    pub fn nodeid(&self) -> &Id {
        &self.nodeid
    }
}