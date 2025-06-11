use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::{
    MAX_ID,
    Id,
};

use crate::core::{
    task::candidate_node::CandidateNode
};

#[derive(Clone)]
pub(crate) struct ClosestSet {
    target: Rc<Id>,
    capacity: usize,

    closest: HashMap<Id, Rc<RefCell<CandidateNode>>>,

    insert_attempt_since_tail_modification: usize,
    insert_attempt_since_head_modification: usize,
}

impl ClosestSet {
    pub(crate) fn new(target: Rc<Id>, capacity: usize) -> Self {
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

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.closest.contains_key(id)
    }

    pub(crate) fn add(&mut self, input: Rc<RefCell<CandidateNode>>) {
        let input_id = input.borrow().id().clone();
        self.closest.insert(input_id.clone(), input);

        if self.closest.len() > self.capacity {
            let last = self.closest.iter().last().unwrap();
            if last.0 == &input_id {
                self.insert_attempt_since_tail_modification += 1;
            } else {
                self.insert_attempt_since_tail_modification = 0;
            }
            self.closest.remove(&last.0.clone());
        }

        if self.closest.len() > 0 {
            let head = self.closest.iter().next().unwrap();
            if head.0 == &input_id {
                self.insert_attempt_since_head_modification = 0;
            } else {
                self.insert_attempt_since_head_modification += 1;
            }
        }
    }

    // pub(crate) fn remove(&mut self, candidate: &Id) {
    //    _ = self.closest.remove(candidate)
    // }

    pub(crate) fn entries(&self) -> Vec<Rc<RefCell<CandidateNode>>> {
        self.closest.values().cloned().collect()
    }

    pub(crate) fn tail(&self) -> Id {
        match self.closest.is_empty() {
            true => self.target.distance(&MAX_ID),
            false => self.closest.iter().last().unwrap().0.clone(),
        }
    }

    /*
    pub(crate) fn head(&self) -> Id {
        match self.closest.is_empty() {
            true => self.target.distance(&MAX_ID),
            false => self.closest.iter().next().unwrap().0.clone(),
        }
    }
    */

    pub(crate) fn is_eligible(&self) -> bool {
        self.reached_capacity() &&
            self.insert_attempt_since_tail_modification > self.capacity
    }
}
