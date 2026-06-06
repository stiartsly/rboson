use std::{
    fmt,
    cmp::Ordering,
    collections::HashSet,
    sync::{Arc, Mutex},
    net::{IpAddr, SocketAddr},
};
use indexmap::map::IndexMap;

use crate::Id;
use crate::dht::task::candidate_node::{
    CandidateNode,
    NodeInfoLike,
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

    fn dedup_key(&self, addr: &SocketAddr) -> DedupKey {
        if self.developer_mode {
            DedupKey::Socket(*addr)
        } else {
            DedupKey::Ip(addr.ip())
        }
    }

    pub(crate) fn add<T>(&mut self, entries: Vec<T>)
    where T: NodeInfoLike
    {
        for item in entries {
            let id = item.id().clone();
            if !self.dedups_ids.insert(id) {
                continue;
            }

            let key = self.dedup_key(item.socket_addr());
            if !self.dedups_addrs.insert(key) {
                self.dedups_ids.remove(item.id());
                continue;
            }

            let cn = Arc::new(Mutex::new(item.into()));
            self.closest.insert_sorted_by(id, cn, |_, a, _, b|
                Self::candidate_order(&self.target, a, b)
            );
        }

        if !self.reached_capacity() {
            return;
        }

        // shrink to fit.
        let mut filtered = self.closest.values().filter(|cn|
            !cn.lock().unwrap().is_inflight()
        ).cloned().collect::<Vec<_>>();

        filtered.sort_by(|a, b|
            Self::candidate_order(&self.target, a, b));

        while self.closest.len() > self.capacity {
            let Some(cn) = filtered.pop() else {
                break;
            };

            let locked_cn = cn.lock().unwrap();
            let id = locked_cn.id();
            if let Some(removed_cn) = self.closest.shift_remove(id) {
                self.dedups_ids.remove(id);

                let locked_cn = removed_cn.lock().unwrap();
                let addr = locked_cn.ni().socket_addr();
                let key = self.dedup_key(addr);
                self.dedups_addrs.remove(&key);

            }
        }
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

    pub(crate) fn remove(&mut self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        if self.is_empty() {
            return None;
        }

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

    #[cfg(test)]
    pub(crate) fn remove_if<F>(&mut self,  _filter: F)
    where F: Fn(&Arc<Mutex<CandidateNode>>) -> bool
    {
        if self.is_empty() {
            return;
        }

        let ids = self.closest.iter().filter_map(|(id, cn)|
            _filter(cn).then_some(id.clone())
        ).collect::<Vec<_>>();

        for id in ids {
            _ = self.closest.shift_remove(&id);
        }
    }

    #[cfg(test)]
    pub(crate) fn candidate_node(&self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        self.closest.get(id).cloned()
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
