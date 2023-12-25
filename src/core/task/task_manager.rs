use std::collections::LinkedList;
use std::rc::Rc;
use std::cell::RefCell;

use super::task::{
    Task,
    State
};

pub(crate) struct TaskManager {
    queued:     LinkedList<Rc<RefCell<Box<dyn Task>>>>,
    running:    LinkedList<Rc<RefCell<Box<dyn Task>>>>,
    canceling: bool,
}

const MAX_ACTIVE_TASKS: usize = 16;

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self {
            queued : LinkedList::new(),
            running: LinkedList::new(),
            canceling: false,
        }
    }

    pub(crate) fn add(&mut self, task: Rc<RefCell<Box<dyn Task>>>) {
        self.add_prior(task, false)
    }

    pub(crate) fn add_prior(&mut self,
        task: Rc<RefCell<Box<dyn Task>>>,
        prior: bool) {
        if self.canceling {
            return;
        }

        if task.borrow().state() == State::Running {
            self.running.push_back(task);
            return;
        }

        let expected = vec![State::Initial];
        if !task.borrow_mut().set_state(&expected, State::Queued) {
            return;
        }

        task.borrow_mut().set_cloned(task.clone());
        match prior {
            true => self.queued.push_front(task),
            false => self.queued.push_back(task),
        }
    }

    // Check whether it's able to dequeue a runnable job from queue.
    #[inline(always)]
    fn can_dequeue(&self) -> bool {
        !(self.canceling ||
            self.running.len() >= MAX_ACTIVE_TASKS ||
            self.queued.is_empty())
    }

    pub(crate) fn dequeue(&mut self) {
        while self.can_dequeue() {
            let task = self.queued.pop_front().unwrap();
            if task.borrow().is_finished() {
                continue;
            }
            if task.borrow().is_canceled() {
                continue;
            }

            task.borrow_mut().start();
            if !task.borrow().is_finished() { // TODO: how can we tackle the jobs on running queue.
                self.running.push_back(task);
            }
        }
    }

    pub(crate) fn cancel_all(&mut self) {
        self.canceling = true;
        while let Some(t) = self.running.pop_front() {
            t.borrow_mut().cancel();
        }
        while let Some(t) = self.queued.pop_front() {
            t.borrow_mut().cancel();
        }
        self.canceling = false;
    }
}
