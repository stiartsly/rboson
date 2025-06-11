use std::rc::Rc;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::cmp::Ordering;
use std::vec::Vec;
use std::collections::HashSet;
use std::collections::HashMap;

use crate::{
    MAX_ID,
    Id,
    NodeInfo
};

use crate::core::{
    task::candidate_node::CandidateNode
};

pub(crate) struct ClosestCandidates {
    target: Rc<Id>,
    capacity: usize,
    dedup_ids: HashSet<Id>,
    dedup_addrs: HashSet<SocketAddr>,
    closest: HashMap<Id, Rc<RefCell<CandidateNode>>>,
}

impl ClosestCandidates {
    pub(crate) fn new(target: Rc<Id>, capacity: usize) -> Self {
        Self {
            target,
            capacity,
            dedup_ids: HashSet::new(),
            dedup_addrs: HashSet::new(),
            closest: HashMap::new(),
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.closest.len()
    }

    pub(crate) fn remove(&mut self, target: &Id) -> Option<Rc<RefCell<CandidateNode>>> {
        let removed = self.closest.remove(target);
        if let Some(cn) = removed.as_ref() {
            self.dedup_ids.remove(target);
            self.dedup_addrs.remove(
                cn.borrow().ni().socket_addr()
            );
        }
        removed
    }

   pub(crate) fn next(&mut self) -> Option<Rc<RefCell<CandidateNode>>> {
        let mut candidated = Vec::with_capacity(self.closest.len());
        self.closest.iter().for_each(|(_, item)| {
            if item.borrow().is_eligible() {
                candidated.push(item.clone());
            }
        });

        candidated.sort_by(|a,b| self.candidate_order(a,b));
        candidated.pop()
    }

    pub(crate) fn head(&self) -> Id {
        match self.closest.is_empty() {
            true => self.target.distance(&MAX_ID),
            false => self.closest.iter().next().unwrap().0.clone()
        }
    }

    /*
    pub(crate) fn tail(&self) -> Id {
        match self.closest.is_empty() {
            true => distance(&self.target, &MAX_ID),
            false => self.closest.iter().last().unwrap().0.clone()
        }
    }*/

    pub(crate) fn add(&mut self, candidates: &[Rc<NodeInfo>]) {
        for item in candidates.iter() {
            if self.dedup_ids.contains(item.id()) {
                continue;
            }
            if self.dedup_addrs.contains(item.socket_addr()) {
                continue;
            }

            self.dedup_ids.insert(item.id().clone());
            self.dedup_addrs.insert(item.socket_addr().clone());

            self.closest.insert(
                item.id().clone(),
                Rc::new(RefCell::new(CandidateNode::new(
                    item.clone(),
                    false
                )))
            );
        }

        if self.closest.len() <= self.capacity {
            return;
        }

        let mut keys: Vec<_> = self.closest.keys().cloned().collect();
        keys.sort_by(|a,b| {
            self.target.three_way_compare(a,b)
        });

        let mut to_remove = Vec::new();
        while let Some(id) = keys.pop() {
            if let Some(item) = self.closest.get(&id) {
                if !item.borrow().is_inflight() {
                    to_remove.push(item.clone());
                }
            }
        }
        to_remove.sort_by(|a,b| self.candidate_order(a, b));
        while self.closest.len() > self.capacity {
            if let Some(item) = to_remove.pop() {
                self.closest.remove(item.borrow().id());
            }
        }

        self.closest.shrink_to(self.capacity);
    }

    fn candidate_order(&self,
        a: &Rc<RefCell<CandidateNode>>,
        b: &Rc<RefCell<CandidateNode>>) -> Ordering
    {
        match a.borrow().pinged().cmp(&b.borrow().pinged()) {
            Ordering::Equal => {
                self.target.three_way_compare(
                    a.borrow().id(),
                    b.borrow().id()
                )
            },
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
        }
    }
}
