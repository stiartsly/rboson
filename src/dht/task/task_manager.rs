use std::{
    sync::{Arc, Mutex},
    sync::atomic::{AtomicBool, Ordering},
    collections::{VecDeque, HashSet},
};
use tokio::sync::Notify;
use log::{debug, error};

use crate::locked;
use crate::dht::{
    consumer::Consumer,
    task::{Task, task::{State, TaskId}}
};

const MAX_ACTIVE_TASKS: usize = 8;

pub(crate) struct TaskManager {
    queued      : Mutex<VecDeque<Arc<Mutex<Box<dyn Task>>>>>,
    running     : Arc<Mutex<HashSet<TaskId>>>,
    canceling   : AtomicBool,

    notifier    : Arc<Notify>,
}

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self {
            queued      : Mutex::new(VecDeque::new()),
            running     : Arc::new(Mutex::new(HashSet::new())),
            canceling   : AtomicBool::new(false),
            notifier    : Arc::new(Notify::new()),
        }
    }

    pub(crate) fn add(&self, task: Box<dyn Task>) {
        self.add_prior(task, false);
        self.notifier.notify_one();
    }

    pub(crate) fn notified(&self) -> Arc<Notify> {
        self.notifier.clone()
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
            Consumer::new(move |_| {
                locked!(running).remove(&taskid);
            })
        );

        assert!(task.is_unstarted());
        if !task.set_state_if(&State::Initialized, State::Queued) {
            error!("!Panic: task is not in Initialized state: {}", task);
			//TODO: call ended handler to avoid task leak
			return;
        }

        self.enqueue(task, priori);
        //self.dequeue(); // call dequeue in rpc_server run_loop.
    }

    #[inline(always)]
    fn is_ready(&self) -> bool {
        !self.canceling.load(Ordering::SeqCst) &&
            locked!(self.running).len() < MAX_ACTIVE_TASKS
    }

    fn enqueue(&self, task: Box<dyn Task>, priori: bool) {
        let task = Arc::new(Mutex::new(task));
        task.lock().unwrap().set_cloned(Arc::downgrade(&task));
        let mut queue = locked!(self.queued);
        match priori {
            true => queue.push_front(task),
            false => queue.push_back(task),
        };
    }

    pub(crate) fn dequeue(&self) {
        while self.is_ready() {
           let Some(task) = locked!(self.queued).pop_front() else {
                debug!("Queue drained.");
                break;
            };

            if task.lock().unwrap().is_ended() {
                continue;
            }

            let taskid = task.lock().unwrap().task_id();
            let _ = locked!(self.running).insert(taskid);

            task.lock().unwrap().start();
        }
    }

    pub(crate) fn stop(&self) {
        self.canceling.store(true, Ordering::SeqCst);

        let _ = locked!(self.running).drain();
        for t in locked!(self.queued).drain(..) {
            t.lock().unwrap().cancel();
        }

        self.canceling.store(false, Ordering::SeqCst);
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        locked!(self.running).clear();
        locked!(self.queued).clear();
    }
}
