use std::net::SocketAddr;
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

pub(crate) struct ClosestCandidates {
    target: Id,
    capacity: usize,
    dedups_ids: HashSet<Id>,
    dedups_addrs: HashSet<SocketAddr>,
    closest: IndexMap<Id, Arc<Mutex<CandidateNode>>>,

    developer_mode: bool,
}

impl ClosestCandidates {
    pub(crate) fn new(target: Id, capacity: usize) -> Self {
        Self {
            target,
            capacity,
            dedups_ids      : HashSet::new(),
            dedups_addrs    : HashSet::new(),
            closest         : IndexMap::new(),
            developer_mode  : false,
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

    pub(crate) fn add<T>(&mut self, mut entries: Vec<T>)
    where
        T: AsCandidateNode
    {
        while let Some(ke) = entries.pop() {
            if self.dedups_ids.contains(ke.id()) {
                continue;
            }
            if self.dedups_addrs.contains(ke.socket_addr()) {
                continue;
            }

            let addr = ke.socket_addr().clone();
            let id = ke.id().clone();
            let cn = Arc::new(Mutex::new(ke.into()));

            self.dedups_ids.insert(id.clone());
            self.dedups_addrs.insert(addr);
            self.closest.insert_sorted_by(id, cn, |_, a,_,b|
                Self::candidate_order(&self.target, a, b)
            );
        }

        self.closest.shrink_to_fit();
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
        if !self.reached_capacity() {
            return;
        }

        let mut keys = self.closest.keys().cloned().collect::<Vec<_>>();
        keys.sort_by(|a,b| self.target.three_way_compare(a,b));

        let mut opted = Vec::new();
        for id in keys {
            let Some(cn) = self.closest.get(&id) else {
                continue;
            };
            if !cn.lock().unwrap().is_inflight() {
                opted.push(cn.clone());
            }
        }
        opted.sort_by(|a,b| Self::candidate_order(&self.target, a, b));

        while self.reached_capacity() {
            let Some(cn) = opted.pop() else {
                break;
            };
            self.closest.shift_remove(cn.lock().unwrap().id());
        }
    }

    pub(crate) fn remove_if<F>(&mut self,  _filter: F)
    where
        F: Fn(&Arc<Mutex<CandidateNode>>) -> bool
    {
        _ = self.closest.pop_if(|_, cn| _filter(cn));
    }

    pub(crate) fn remove(&mut self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        // Retain dedup to prevent re-addition of the same node.
        self.closest.shift_remove(id)
    }

    pub(crate) fn next(&self) -> Option<Arc<Mutex<CandidateNode>>> {
        let mut todo = self.closest.values().filter(|cn|
            cn.lock().unwrap().is_eligible()
        ).cloned().collect::<Vec<_>>();

        if todo.is_empty() {
            return None;
        }

        todo.sort_by(|a,b| Self::candidate_order(&self.target, a, b));
        todo.pop()
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
