use std::{
    rc::Rc,
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
    collections::{VecDeque, HashSet},
};
use log::{debug, error};

use crate::dht::{
    handler::Handler,
    task::{Task, task::{State, TaskId}}
};

const MAX_ACTIVE_TASKS: usize = 8;

pub(crate) struct TaskManager {
    queued      : RefCell<VecDeque<Rc<RefCell<Box<dyn Task>>>>>,
    running     : RefCell<HashSet<TaskId>>,
    canceling   : AtomicBool,
}

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self {
            queued      : RefCell::new(VecDeque::new()),
            running     : RefCell::new(HashSet::new()),
            canceling   : AtomicBool::new(false),
        }
    }

    pub(crate) fn add(&self, task: Box<dyn Task>) {
        self.add_prior(task, false);
    }

    pub(crate) fn add_prior(&self, mut task: Box<dyn Task>, priori: bool) {
        if self.canceling.load(Ordering::SeqCst) {
            return;
        }
        if task.is_ended() {
            return;
        }

        let taskid = task.task_id();
        let running = self.running.clone();
        task.with_ended_handler(
            Handler::new(move |_| {
                running.borrow_mut().remove(&taskid);
            })
        );

        assert!(task.is_unstarted());
        if !task.set_state_if(&State::Initialized, State::Queued) {
            error!("!Panic: task is not in Initialized state: {}", task);
			//TODO: call ended handler to avoid task leak
			return;
        }

        self.enqueue(task, priori);
        self.dequeue();
    }

    #[inline(always)]
    fn is_ready(&self) -> bool {
        !self.canceling.load(Ordering::SeqCst) &&
            self.running.borrow().len() < MAX_ACTIVE_TASKS
    }

    fn enqueue(&self, task: Box<dyn Task>, priori: bool) {
        let task = Rc::new(RefCell::new(task));
        task.borrow_mut().set_cloned(std::rc::Rc::downgrade(&task));
        let mut queue = self.queued.borrow_mut();
        match priori {
            true => queue.push_front(task),
            false => queue.push_back(task),
        };
    }

    pub(crate) fn dequeue(&self) {
        while self.is_ready() {
           let Some(task) = self.queued.borrow_mut().pop_front() else {
                debug!("Queue drained.");
                break;
            };

            if task.borrow().is_ended() {
                continue;
            }

            let taskid = task.borrow().task_id();
            let _ = self.running.borrow_mut().insert(taskid);

            task.borrow_mut().start();
        }
    }

    pub(crate) fn stop(&self) {
        self.canceling.store(true, Ordering::SeqCst);

        let _ = self.running.borrow_mut().drain();
        for t in self.queued.borrow_mut().drain(..) {
            t.borrow_mut().cancel();
        }

        self.canceling.store(false, Ordering::SeqCst);
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        self.running.borrow_mut().clear();
        self.queued.borrow_mut().clear();
    }
}
