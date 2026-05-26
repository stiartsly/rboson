use crate::{
    Network,
    NodeInfo,
};

pub(crate) struct Data {
    pub(crate) nodes4  : Option<Vec<NodeInfo>>,
    pub(crate) nodes6  : Option<Vec<NodeInfo>>,
    pub(crate) token   : i32,
}

impl Data {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>,
        token: i32
    ) -> Self {
        Self { nodes4, nodes6, token }
    }
}

pub(crate) trait LookupResponse {
    fn data(&self) -> &Data;
    //fn data_mut(&mut self) -> &mut Data;

    fn nodes4(&self) -> Option<&[NodeInfo]> {
        self.data().nodes4.as_deref()
    }

    fn nodes6(&self) -> Option<&[NodeInfo]> {
        self.data().nodes6.as_deref()
    }

    fn nodes(&self, network: Network) -> Option<&[NodeInfo]> {
        match network {
            Network::IPv4 => self.nodes4(),
            Network::IPv6 => self.nodes6(),
        }
    }

    fn token(&self) -> i32 {
        self.data().token
    }
}
