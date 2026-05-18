use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use crate::{Id, NodeInfo};
use crate::dht::{
    routing::kbucket_entry::KBucketEntry,
    task::candidate_node::CandidateNode,
};

pub(crate) trait Reachability {
    fn is_reachable(&self) -> bool { false }
    fn is_unreachable(&self) -> bool { false }
    fn set_reachable(&mut self, _: bool) {}
}

pub(crate) enum NodeEntry {
    CandidateNode(Arc<Mutex<CandidateNode>>),
    KBucketEntry(KBucketEntry),
    NodeInfo(NodeInfo),
}

impl NodeEntry {
    pub(crate) fn from_candidate(candidate: Arc<Mutex<CandidateNode>>) -> Self {
        Self::CandidateNode(candidate)
    }

    pub(crate) fn from_kentry(entry: KBucketEntry) -> Self {
        Self::KBucketEntry(entry)
    }

    pub(crate) fn from_node(node_info: NodeInfo) -> Self {
        Self::NodeInfo(node_info)
    }

    pub(crate) fn id(&self) -> Id {
        match self {
            NodeEntry::CandidateNode(v) => v.lock().unwrap().id().clone(),
            NodeEntry::KBucketEntry(v) => v.id().clone(),
            NodeEntry::NodeInfo(v) => v.id().clone(),
        }
    }

    pub(crate) fn socket_addr(&self) -> SocketAddr {
        match self {
            NodeEntry::CandidateNode(v) => v.lock().unwrap().as_ref().socket_addr().clone(),
            NodeEntry::KBucketEntry(v) => v.socket_addr().clone(),
            NodeEntry::NodeInfo(v) => v.socket_addr().clone(),
        }
    }
}

impl Reachability for NodeEntry {
    fn is_reachable(&self) -> bool {
        match self {
            NodeEntry::CandidateNode(v) => v.lock().unwrap().is_reachable(),
            NodeEntry::KBucketEntry(v) => v.is_reachable(),
            _ => false,
        }
    }

    fn is_unreachable(&self) -> bool {
        match self {
            NodeEntry::CandidateNode(v) => v.lock().unwrap().is_unreachable(),
            NodeEntry::KBucketEntry(v) => v.is_unreachable(),
            _ => false,
        }
    }

    fn set_reachable(&mut self, reachable: bool) {
        match self {
            NodeEntry::CandidateNode(v) => v.lock().unwrap().set_reachable(reachable),
            NodeEntry::KBucketEntry(v) => v.set_reachable(reachable),
            _ => {}
        }
    }
}

