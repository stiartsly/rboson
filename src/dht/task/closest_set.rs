use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use log::debug;

use crate::Id;
use crate::dht::{
    task::candidate_node::CandidateNode,
};

#[derive(Clone)]
pub(crate) struct ClosestSet {
    target: Id,
    capacity: usize,

    closest: HashMap<Id, Arc<Mutex<CandidateNode>>>,

    insert_attempt_since_tail_modification: usize,
    insert_attempt_since_head_modification: usize,
}

impl ClosestSet {
    pub(crate) fn new(target: Id, capacity: usize) -> Self {
        Self {
            target,
            capacity,
            closest: HashMap::new(),
            insert_attempt_since_tail_modification: 0,
            insert_attempt_since_head_modification: 0,
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

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.closest.contains_key(id)
    }

    pub(crate) fn add(&mut self, cn: Arc<Mutex<CandidateNode>>) {
        let id = cn.lock().unwrap().id().clone();
        self.closest.insert(id, cn);

        let exceeded = self.closest.len() > self.capacity;
        if exceeded {
            let last_id = self.closest.iter().last().unwrap().0.clone();
            _ = self.closest.remove(&last_id);

            if last_id == id {
                self.insert_attempt_since_tail_modification += 1;
            } else {
                self.insert_attempt_since_tail_modification = 0;
            }

            debug!("Removed farthest candidate {}, tail modification count: {}",
                last_id, self.insert_attempt_since_tail_modification
            );
        }

        if self.closest.len() > 0 {
            let head = self.closest.iter().next().unwrap();
            if head.0 == &id {
                self.insert_attempt_since_head_modification = 0;
            } else {
                self.insert_attempt_since_head_modification += 1;
            }
        }
    }

    // pub(crate) fn remove(&mut self, candidate: &Id) {
    //    _ = self.closest.remove(candidate)
    // }

    pub(crate) fn entries(&self) -> Vec<Arc<Mutex<CandidateNode>>> {
        self.closest.values().cloned().collect()
    }

    pub(crate) fn tail(&self) -> Id {
        match self.closest.is_empty() {
            true => self.target.distance(&Id::MAX_ID),
            false => self.closest.iter().last().unwrap().0.clone(),
        }
    }

    /*
    pub(crate) fn head(&self) -> Id {
        match self.closest.is_empty() {
            true => self.target.distance(&Id::MAX_ID),
            false => self.closest.iter().next().unwrap().0.clone(),
        }
    }
    */

    pub(crate) fn is_eligible(&self) -> bool {
        self.reached_capacity() &&
            self.insert_attempt_since_tail_modification > self.capacity
    }
}
