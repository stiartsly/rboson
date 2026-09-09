use std::{
    rc::Rc,
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
    collections::{HashMap, VecDeque},
};
use log::{debug, error};

use crate::EasyHandler;
use crate::dht::{
    task::{Task, task::{State, TaskId}}
};

const MAX_ACTIVE_TASKS: usize = 8;

pub(crate) struct TaskManager {
    queued      : RefCell<VecDeque<Rc<RefCell<Box<dyn Task>>>>>,
    running     : Rc<RefCell<HashMap<TaskId, Rc<RefCell<Box<dyn Task>>>>>>,
    canceling   : AtomicBool,
}

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self {
            queued      : RefCell::new(VecDeque::new()),
            running     : Rc::new(RefCell::new(HashMap::new())),
            canceling   : AtomicBool::new(false),
        }
    }

    pub(crate) fn add(self: &Rc<Self>, task: Box<dyn Task>) {
        self.add_prior(task, false);
    }

    pub(crate) fn add_prior(self: &Rc<Self>, mut task: Box<dyn Task>, priori: bool) {
        if self.canceling.load(Ordering::SeqCst) {
            return;
        }
        if task.is_ended() {
            return;
        }

        let taskid = task.task_id();
        let manager = Rc::downgrade(self);
        task.with_ended_handler(
            EasyHandler ::new(move |_| {
                let Some(manager) = manager.upgrade() else {
                    return;
                };

                let _ = manager.running.borrow_mut().remove(&taskid);
                manager.dequeue();
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
        task.borrow_mut().set_cloned(Rc::downgrade(&task));
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
            self.running.borrow_mut().insert(taskid, task.clone());
            task.borrow_mut().start();
        }
    }

    pub(crate) fn stop(&self) {
        self.canceling.store(true, Ordering::SeqCst);

        let running = self.running.borrow_mut().drain()
            .map(|(_, task)| task)
            .collect::<Vec<_>>();
        let queued = self.queued.borrow_mut().drain(..).collect::<Vec<_>>();

        for t in running.into_iter().chain(queued) {
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
