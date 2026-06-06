use std::{
    time::SystemTime,
    net::SocketAddr
};
use crate::{Id, NodeInfo};
use crate::dht::{
    rpc::Reachability,
    routing::KBucketEntry
};

const MAX_PINGS: i32 = 3;

#[derive(Clone)]
pub(crate) struct CandidateNode {
    ni: NodeInfo,

    last_sent   : Option<SystemTime>,
    last_replied: Option<SystemTime>,

    acked: bool,
    pinged: i32,

    reachable: bool,
    token: i32,
}

impl CandidateNode {
    fn new(ni: NodeInfo, reachable: bool) -> Self {
        Self {
            ni,
            last_sent: None,
            last_replied: None,
            acked: false,
            pinged: 0,
            reachable,
            token: 0,
        }
    }

    pub(crate) fn id(&self) -> &Id {
        self.ni.id()
    }

    pub(crate) fn ni(&self) -> &NodeInfo {
        &self.ni
    }

    pub(crate) fn set_sent(&mut self) {
        self.last_sent = Some(SystemTime::now());
        self.pinged += 1;
    }

    pub(crate) fn clear_sent(&mut self) {
        self.last_sent = None;
    }

    #[allow(unused)]
    pub(crate) fn is_sent(&self) -> bool {
        self.last_sent.is_some()
    }

    pub(crate) fn pinged(&self) -> i32 {
        self.pinged
    }

    pub(crate) fn set_replied(&mut self) {
        self.last_sent = None;
        self.last_replied = Some(SystemTime::now());
    }

    #[allow(unused)]
    pub(crate) fn is_replied(&self) -> bool {
        self.last_replied.is_some()
    }

    pub(crate) fn set_token(&mut self, token: i32) {
        self.token = token
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    #[allow(unused)]
    pub(crate) fn set_acked(&mut self) {
        self.acked = true;
    }

    #[allow(unused)]
    pub(crate) fn is_acked(&self) -> bool {
        self.acked
    }

    pub(crate) fn is_inflight(&self) -> bool {
        self.last_sent.is_some()
    }

    pub(crate) fn is_eligible(&self) -> bool {
        self.last_sent.is_none() && self.pinged < MAX_PINGS
    }
}

impl From<NodeInfo> for CandidateNode {
    fn from(ni: NodeInfo) -> Self {
        Self::new(ni, false)
    }
}

impl From<KBucketEntry> for CandidateNode {
    fn from(entry: KBucketEntry) -> Self {
        Self::new(entry.ni().clone(), entry.is_reachable())
    }
}

impl Reachability for CandidateNode {
    fn is_reachable(&self) -> bool {
        self.reachable
    }

    fn is_unreachable(&self) -> bool {
        self.pinged >= MAX_PINGS
    }

    fn set_reachable(&mut self, reachable: bool) {
        self.reachable = reachable
    }
}

pub(crate) trait NodeInfoLike: Into<CandidateNode> {
    fn id(&self) -> &Id;
    fn socket_addr(&self) -> &SocketAddr;
}

impl NodeInfoLike for NodeInfo {
    fn id(&self) -> &Id {
        self.id()
    }

    fn socket_addr(&self) -> &SocketAddr {
        self.socket_addr()
    }
}

impl NodeInfoLike for KBucketEntry {
    fn id(&self) -> &Id {
        self.as_ref().id()
    }

    fn socket_addr(&self) -> &SocketAddr {
        self.as_ref().socket_addr()
    }
}
