use std::collections::LinkedList;
use std::sync::{Arc, Mutex};

use super::task::{Task, State};

const MAX_ACTIVE_TASKS: usize = 16;

pub(crate) struct TaskManager {
    queued:     LinkedList<Arc<Mutex<dyn Task>>>,
    running:    LinkedList<Arc<Mutex<dyn Task>>>,
    canceling: bool,
}

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self {
            queued : LinkedList::new(),
            running: LinkedList::new(),
            canceling: false,
        }
    }

    pub(crate) fn add(&mut self, task: Arc<Mutex<Box<dyn Task>>>) {
        //self.add_prior(task, false)
    }

    pub(crate) fn add_prior(&mut self,
        task: Arc<Mutex<dyn Task>>,
        prior: bool) {
        if self.canceling {
            return;
        }

        if task.lock().unwrap().state() == State::Running {
            self.running.push_back(task);
            return;
        }

        let expected = vec![State::Initial];
       // if !task.lock().unwrap().set_state(&expected, State::Queued) {
       //     return;
       // }

       // task.lock().unwrap().set_cloned(task.clone());
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
            if task.lock().unwrap().is_complete() {
                continue;
            }
            if task.lock().unwrap().is_canceled() {
                continue;
            }

            task.lock().unwrap().start();
            if !task.lock().unwrap().is_complete() { // TODO: how can we tackle the jobs on running queue.
                self.running.push_back(task);
            }
        }
    }

    pub(crate) fn cancel_all(&mut self) {
        self.canceling = true;
        while let Some(t) = self.running.pop_front() {
            t.lock().unwrap().cancel();
        }
        while let Some(t) = self.queued.pop_front() {
            t.lock().unwrap().cancel();
        }
        self.canceling = false;
    }
}
