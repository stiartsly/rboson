use std::{
    net::SocketAddr,
    sync::{Arc, Mutex}
};

use crate::{Id, NodeInfo};
use crate::dht::{
    routing::KBucketEntry,
    task::CandidateNode,
};

impl Into<Target> for NodeInfo {
    fn into(self) -> Target {
        Target::NodeInfo(self)
    }
}

impl Into<Target> for KBucketEntry {
    fn into(self) -> Target {
        Target::KBucketEntry(self)
    }
}

impl Into<Target> for Arc<Mutex<CandidateNode>> {
    fn into(self) -> Target {
        Target::Candidate(self)
    }
}

pub(crate) trait Reachability {
    fn is_reachable(&self) -> bool { false }
    fn is_unreachable(&self) -> bool { false }
    fn set_reachable(&mut self, _: bool) {}
}

#[allow(unused)]
pub(crate) trait NodeInfoLike {
    fn ni(&self) -> NodeInfo;
    fn id(&self) -> &Id;
    fn socket_addr(&self) -> &SocketAddr;
}

pub(crate) enum Target {
    Candidate(Arc<Mutex<CandidateNode>>),
    KBucketEntry(KBucketEntry),
    NodeInfo(NodeInfo),
}

impl Target {
    pub(crate) fn ni(&self) -> NodeInfo {
        match self {
            Target::Candidate(v) => v.lock().unwrap().ni(),
            Target::KBucketEntry(v) => v.ni(),
            Target::NodeInfo(v) => v.ni()
        }
    }

    pub(crate) fn id(&self) -> Id {
        match self {
            Target::Candidate(v) => *v.lock().unwrap().id(),
            Target::KBucketEntry(v) => *v.id(),
            Target::NodeInfo(v) => *v.id()
        }
    }
}

impl Reachability for Target {
    fn is_reachable(&self) -> bool {
        match self {
            Target::Candidate(v) => v.lock().unwrap().is_reachable(),
            Target::KBucketEntry(v) => v.is_reachable(),
            _ => false,
        }
    }

    fn is_unreachable(&self) -> bool {
        match self {
            Target::Candidate(v) => v.lock().unwrap().is_unreachable(),
            Target::KBucketEntry(v) => v.is_unreachable(),
            _ => false,
        }
    }

    fn set_reachable(&mut self, reachable: bool) {
        match self {
            Target::Candidate(v) => v.lock().unwrap().set_reachable(reachable),
            Target::KBucketEntry(v) => v.set_reachable(reachable),
            _ => {}
        }
    }
}

impl Reachability for NodeInfo {}

impl NodeInfoLike for NodeInfo {
    fn ni(&self) -> NodeInfo {
        self.clone()
    }

    fn id(&self) -> &Id {
        self.id()
    }

    fn socket_addr(&self) -> &SocketAddr {
        self.socket_addr()
    }
}
