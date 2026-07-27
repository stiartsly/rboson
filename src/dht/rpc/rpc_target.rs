use std::{
    rc::Rc,
    cell::RefCell,
    net::SocketAddr
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

impl Into<Target> for Rc<RefCell<CandidateNode>> {
    fn into(self) -> Target {
        Target::Candidate(self)
    }
}

pub(crate) trait TargetInfo {
    fn is_reachable(&self) -> bool { false }
    fn is_unreachable(&self) -> bool { false }
    fn set_reachable(&mut self, _: bool) {}

    fn ni(&self) -> NodeInfo;
    fn addr(&self) -> &SocketAddr;
}

#[derive(Clone)]
pub(crate) enum Target {
    Candidate(Rc<RefCell<CandidateNode>>),
    KBucketEntry(KBucketEntry),
    NodeInfo(NodeInfo),
}

impl Target {
    /*
    pub(crate) fn ni(&self) -> NodeInfo {
        match self {
            Target::Candidate(v) => v.borrow().ni(),
            Target::KBucketEntry(v) => v.ni(),
            Target::NodeInfo(v) => v.ni()
        }
    }
    */

    pub(crate) fn id(&self) -> Id {
        match self {
            Target::Candidate(v) => *v.borrow().id(),
            Target::KBucketEntry(v) => *v.id(),
            Target::NodeInfo(v) => *v.id()
        }
    }

    #[allow(unused)]
    pub(crate) fn socket_addr(&self) -> SocketAddr {
        match self {
            Target::Candidate(v) => *v.borrow().addr(),
            Target::KBucketEntry(v) => *v.addr(),
            Target::NodeInfo(v) => *v.address()
        }
    }
}

impl TargetInfo for Target {
    fn is_reachable(&self) -> bool {
        match self {
            Target::Candidate(v) => v.borrow().is_reachable(),
            Target::KBucketEntry(v) => v.is_reachable(),
            _ => false,
        }
    }

    fn is_unreachable(&self) -> bool {
        match self {
            Target::Candidate(v) => v.borrow().is_unreachable(),
            Target::KBucketEntry(v) => v.is_unreachable(),
            _ => false,
        }
    }

    fn set_reachable(&mut self, reachable: bool) {
        match self {
            Target::Candidate(v) => v.borrow_mut().set_reachable(reachable),
            Target::KBucketEntry(v) => v.set_reachable(reachable),
            _ => {}
        }
    }

    fn ni(&self) -> NodeInfo {
        match self {
            Target::Candidate(v) => v.borrow().ni(),
            Target::KBucketEntry(v) => v.ni(),
            Target::NodeInfo(v) => v.clone(),
        }
    }

    fn addr(&self) -> &SocketAddr {
        match self {
            Target::Candidate(_v) => {
                panic!("N/A");
                #[allow(unused)]_v.borrow().addr()
            },
            Target::KBucketEntry(v) => v.addr(),
            Target::NodeInfo(v) => v.address(),
        }
    }
}

impl TargetInfo for NodeInfo {
    fn ni(&self) -> NodeInfo {
        self.clone()
    }
    fn addr(&self) -> &SocketAddr {
        self.address()
    }
}
