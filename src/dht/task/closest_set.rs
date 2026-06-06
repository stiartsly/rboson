use std::{
    fmt,
    cmp::Ordering,
    sync::{Arc, Mutex}
};
use indexmap::map::IndexMap;
use log::debug;

use crate::Id;
use crate::dht::task::candidate_node::CandidateNode;

#[derive(Clone)]
pub(crate) struct ClosestSet {
    target: Id,
    capacity: usize,

    closest: IndexMap<Id, Arc<Mutex<CandidateNode>>>,

    insert_attempt_since_tail_modification: usize,
    insert_attempt_since_head_modification: usize,
}

impl ClosestSet {
    pub(crate) fn new(target: Id, capacity: usize) -> Self {
        Self {
            target,
            capacity,
            closest: IndexMap::new(),
            insert_attempt_since_tail_modification: 0,
            insert_attempt_since_head_modification: 0,
        }
    }

    pub(crate) fn reached_capacity(&self) -> bool {
        self.closest.len() >= self.capacity
    }

    #[cfg(test)]
    pub(crate) fn size(&self) -> usize {
        self.closest.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.closest.is_empty()
    }

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.closest.contains_key(id)
    }

    fn candidate_order(
        target: &Id,
        left: &Arc<Mutex<CandidateNode>>,
        right: &Arc<Mutex<CandidateNode>>,
    ) -> Ordering {
        let left_id = left.lock().unwrap().id().clone();
        let right_id = right.lock().unwrap().id().clone();
        target.three_way_compare(&left_id, &right_id)
    }

    pub(crate) fn add(&mut self, cn: Arc<Mutex<CandidateNode>>) {
        let id = cn.lock().unwrap().id().clone();
        self.closest.insert_sorted_by(id, cn, |_, left, _, right|
            Self::candidate_order(&self.target, left, right)
        );

        debug!("Added candidate {} to ClosestSet, size now {}", id, self.closest.len());

        if self.closest.len() > self.capacity {
            let last_id = self.closest.last().unwrap().0.clone();
            _ = self.closest.shift_remove(&last_id);

            if last_id == id {
                self.insert_attempt_since_tail_modification += 1;
            } else {
                self.insert_attempt_since_tail_modification = 0;
            }

            debug!("Removed farthest candidate {}, tail modification count: {}",
                last_id, self.insert_attempt_since_tail_modification
            );
        }

        if let Some((head_id, _)) = self.closest.first() {
            if head_id == &id {
                self.insert_attempt_since_head_modification = 0;
            } else {
                self.insert_attempt_since_head_modification += 1;
            }
        }
    }

    pub(crate) fn entries(&self) -> Vec<Arc<Mutex<CandidateNode>>> {
        self.closest.values().cloned().collect()
    }

    pub(crate) fn head(&self) -> Id {
        match self.closest.first() {
            Some((id, _)) => id.clone(),
            None => self.target.distance(&Id::MAX_ID),
        }
    }

    pub(crate) fn tail(&self) -> Id {
        match self.closest.last() {
            Some((id, _)) => id.clone(),
            None => self.target.distance(&Id::MAX_ID),
        }
    }

    pub(crate) fn is_eligible(&self) -> bool {
        self.reached_capacity() &&
            self.insert_attempt_since_tail_modification > self.capacity
    }

    #[cfg(test)]
    pub(crate) fn is_head_stable(&self) -> bool {
        self.insert_attempt_since_head_modification > self.capacity
    }

    #[cfg(test)]
    pub(crate) fn insert_attempts_since_tail_modification(&self) -> usize {
        self.insert_attempt_since_tail_modification
    }

    #[cfg(test)]
    pub(crate) fn insert_attempts_since_head_modification(&self) -> usize {
        self.insert_attempt_since_head_modification
    }

    #[cfg(test)]
    pub(crate) fn entry(&self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        self.closest.get(id).cloned()
    }

    #[cfg(test)]
    pub(crate) fn remove(&mut self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        if self.is_empty() {
            return None
        }
        self.closest.shift_remove(id)
    }
}

impl fmt::Display for ClosestSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClosestSet: size={} head={} tail={}",
            self.closest.len(),
            self.head(),
            self.tail(),
        )
    }
}
