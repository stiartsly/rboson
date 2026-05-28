use std::time::SystemTime;
use std::net::SocketAddr;

use crate::{Id, NodeInfo};
use crate::dht::{
    rpc::rpc_target::Reachability,
    routing::kbucket_entry::KBucketEntry
};

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
            last_sent:      None,
            last_replied:   None,
            pinged: 0,
            acked: false,
            reachable,
            token: 0,
        }
    }

    pub(crate) fn id(&self) -> &Id {
        self.ni.id()
    }

    pub(crate) fn set_sent(&mut self) {
        self.last_sent = Some(SystemTime::now());
        self.pinged += 1;
    }

    pub(crate) fn clear_sent(&mut self) {
        self.last_sent = None;
    }

    pub(crate) fn is_sent(&self) -> bool {
        self.last_sent.is_some()
    }

    pub(crate) fn pinged(&self) -> i32 {
        self.pinged
    }

    pub(crate) fn set_replied(&mut self) {
        self.last_replied = Some(SystemTime::now());
    }

    pub(crate) fn is_replied(&self) -> bool {
        self.last_replied.is_some()
    }

    pub(crate) fn set_token(&mut self, token: i32) {
        self.token = token
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn set_acked(&mut self) {
        self.acked = true;
    }

    pub(crate) fn is_acked(&self) -> bool {
        self.acked
    }

    pub(crate) fn is_inflight(&self) -> bool {
        self.last_sent.is_some()
    }

    pub(crate) fn is_eligible(&self) -> bool {
        self.last_sent.is_none() && self.pinged < 3
    }
}

impl From<NodeInfo> for CandidateNode {
    fn from(ni: NodeInfo) -> Self {
        Self::new(ni, false)
    }
}

impl From<KBucketEntry> for CandidateNode {
    fn from(entry: KBucketEntry) -> Self {
        Self::new(
            entry.as_ref().clone(),
            entry.is_reachable()
        )
    }
}

impl AsRef<NodeInfo> for CandidateNode {
    fn as_ref(&self) -> &NodeInfo {
        &self.ni
    }
}

impl Reachability for CandidateNode {
    fn is_reachable(&self) -> bool {
        self.reachable
    }

    fn is_unreachable(&self) -> bool {
        self.pinged >= 3
    }

    fn set_reachable(&mut self, reachable: bool) {
        self.reachable = reachable
    }
}

pub(crate) trait AsCandidateNode: Into<CandidateNode> {
    fn id(&self) -> &Id;
    fn socket_addr(&self) -> &SocketAddr;
}

impl AsCandidateNode for NodeInfo {
    fn id(&self) -> &Id {
        self.id()
    }

    fn socket_addr(&self) -> &SocketAddr {
        self.socket_addr()
    }
}

impl AsCandidateNode for KBucketEntry {
    fn id(&self) -> &Id {
        self.as_ref().id()
    }

    fn socket_addr(&self) -> &SocketAddr {
        self.as_ref().socket_addr()
    }
}
