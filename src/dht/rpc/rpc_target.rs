use std::{
    net::SocketAddr,
    sync::{Arc, Mutex}
};

use crate::{Id, NodeInfo};
use crate::dht::{
    routing::KBucketEntry,
    task::CandidateNode,
};

pub(crate) trait Reachability {
    fn is_reachable(&self) -> bool { false }
    fn is_unreachable(&self) -> bool { false }
    fn set_reachable(&mut self, _: bool) {}
}

pub(crate) enum Target {
    Candidate(Arc<Mutex<CandidateNode>>),
    KBucketEntry(KBucketEntry),
    NodeInfo(NodeInfo),
}

impl Target {
    pub(crate) fn from_candidate(candidate: Arc<Mutex<CandidateNode>>) -> Self {
        Self::Candidate(candidate)
    }

    pub(crate) fn from_bucket_entry(entry: KBucketEntry) -> Self {
        Self::KBucketEntry(entry)
    }

    pub(crate) fn node_info(&self) -> NodeInfo {
        match self {
            Target::Candidate(v) => v.lock().unwrap().as_ref().clone(),
            Target::KBucketEntry(v) => v.as_ref().clone(),
            Target::NodeInfo(v) => v.clone(),
        }
    }

    pub(crate) fn id(&self) -> Id {
        self.node_info().id().clone()
    }

    pub(crate) fn socket_addr(&self) -> SocketAddr {
        *self.node_info().socket_addr()
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

