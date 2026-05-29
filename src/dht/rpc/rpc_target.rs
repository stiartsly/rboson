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

    pub(crate) fn id(&self) -> Id {
        match self {
            Target::Candidate(v) => v.lock().unwrap().id().clone(),
            Target::KBucketEntry(v) => v.id().clone(),
            Target::NodeInfo(v) => v.id().clone(),
        }
    }

    pub(crate) fn socket_addr(&self) -> SocketAddr {
        match self {
            Target::Candidate(v) => v.lock().unwrap().as_ref().socket_addr().clone(),
            Target::KBucketEntry(v) => v.socket_addr().clone(),
            Target::NodeInfo(v) => v.socket_addr().clone(),
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

