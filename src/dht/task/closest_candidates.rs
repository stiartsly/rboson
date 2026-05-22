use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::cmp::Ordering;
use std::collections::HashSet;
use indexmap::map::IndexMap;
use std::sync::{Arc, Mutex};

use crate::Id;
use crate::dht::{
    task::candidate_node::{
        CandidateNode,
        AsCandidateNode,
    }
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum DedupKey {
    Ip(IpAddr),
    Socket(SocketAddr),
}

pub(crate) struct ClosestCandidates {
    target: Id,
    capacity: usize,
    dedups_ids: HashSet<Id>,
    dedups_addrs: HashSet<DedupKey>,
    closest: IndexMap<Id, Arc<Mutex<CandidateNode>>>,

    developer_mode: bool,
}

impl ClosestCandidates {
    pub(crate) fn new(target: Id, capacity: usize) -> Self {
        Self::with_developer_mode(target, capacity, false)
    }

    pub(crate) fn with_developer_mode(target: Id, capacity: usize, developer_mode: bool) -> Self {
        Self {
            target,
            capacity,
            dedups_ids      : HashSet::new(),
            dedups_addrs    : HashSet::new(),
            closest         : IndexMap::new(),
            developer_mode,
        }
    }

    pub(crate) fn reached_capacity(&self) -> bool {
        self.closest.len() >= self.capacity
    }

    pub(crate) fn size(&self) -> usize {
        self.closest.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.closest.is_empty()
    }

    pub(crate) fn get(&self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        self.closest.get(id).cloned()
    }

    fn dedup_key(&self, addr: &SocketAddr) -> DedupKey {
        if self.developer_mode {
            DedupKey::Socket(*addr)
        } else {
            DedupKey::Ip(addr.ip())
        }
    }

    fn remove_from_dedup(&mut self, id: &Id, candidate: &Arc<Mutex<CandidateNode>>) {
        self.dedups_ids.remove(id);

        let addr = *candidate.lock().unwrap().as_ref().socket_addr();
        self.dedups_addrs.remove(&self.dedup_key(&addr));
    }

    pub(crate) fn add<T>(&mut self, entries: Vec<T>)
    where
        T: AsCandidateNode
    {
        for ke in entries {
            if !self.dedups_ids.insert(ke.id().clone()) {
                continue;
            }

            let addr_key = self.dedup_key(ke.socket_addr());
            if !self.dedups_addrs.insert(addr_key) {
                self.dedups_ids.remove(ke.id());
                continue;
            }

            let id = ke.id().clone();
            let cn = Arc::new(Mutex::new(ke.into()));

            self.closest.insert_sorted_by(id, cn, |_, a, _, b|
                Self::candidate_order(&self.target, a, b)
            );
        }

        self.shrink_to_fit();
    }

    fn candidate_order(
        target: &Id,
        a: &Arc<Mutex<CandidateNode>>,
        b: &Arc<Mutex<CandidateNode>>
    ) -> Ordering {
        let id_a = a.lock().unwrap().id().clone();
        let id_b = b.lock().unwrap().id().clone();

        match target.three_way_compare(&id_a, &id_b) {
            Ordering::Less      => Ordering::Less,
            Ordering::Greater   => Ordering::Greater,
            Ordering::Equal     => {
                let pinged_a = a.lock().unwrap().pinged();
                let pinged_b = b.lock().unwrap().pinged();
                pinged_a.cmp(&pinged_b)
            }
        }
    }

    fn shrink_to_fit(&mut self) {
        if self.closest.len() <= self.capacity {
            return;
        }

        let mut opted = self.closest.values().filter(|cn|
            !cn.lock().unwrap().is_inflight()
        ).cloned().collect::<Vec<_>>();
        opted.sort_by(|a, b| Self::candidate_order(&self.target, a, b));

        while self.closest.len() > self.capacity {
            let Some(cn) = opted.pop() else {
                break;
            };

            let id = cn.lock().unwrap().id().clone();
            if let Some(removed) = self.closest.shift_remove(&id) {
                self.remove_from_dedup(&id, &removed);
            }
        }
    }

    pub(crate) fn remove_if<F>(&mut self,  _filter: F)
    where
        F: Fn(&Arc<Mutex<CandidateNode>>) -> bool
    {
        let ids = self.closest.iter().filter_map(|(id, cn)|
            _filter(cn).then_some(id.clone())
        ).collect::<Vec<_>>();

        for id in ids {
            _ = self.closest.shift_remove(&id);
        }
    }

    pub(crate) fn remove(&mut self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        // Retain dedup to prevent re-addition of the same node.
        self.closest.shift_remove(id)
    }

    pub(crate) fn next(&self) -> Option<Arc<Mutex<CandidateNode>>> {
        self.closest.values().filter(|cn|
            cn.lock().unwrap().is_eligible()
        ).min_by(|left, right|
            Self::candidate_order(&self.target, left, right)
        ).cloned()
    }

    pub(crate) fn ids(&self) -> Vec<Id> {
        self.closest.keys().cloned().collect()
    }

    pub(crate) fn entries(&self) -> Vec<Arc<Mutex<CandidateNode>>> {
        self.closest.values().cloned().collect()
    }

    pub(crate) fn tail(&self) -> Id {
        match self.closest.last() {
            Some((id, _)) => id.clone(),
            None => self.target.distance(&Id::MAX_ID)
        }
    }

    pub(crate) fn head(&self) -> Id {
        match self.closest.first() {
            Some((id, _)) => id.clone(),
            None => self.target.distance(&Id::MAX_ID)
        }
    }
}

impl fmt::Display for ClosestCandidates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClosestCandidates: size={} head={} tail={}",
            self.closest.len(),
            self.head(),
            self.tail(),
        )
    }
}
