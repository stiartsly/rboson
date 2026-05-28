use std::{
    sync::{Arc, Mutex},
    sync::atomic::{AtomicBool, Ordering},
    collections::{VecDeque, HashMap},
};
use tokio::task;
use log::{debug, error};

use crate::dht::{
    consumer::Consumer,
    task::task::{Task, TaskId, State},
};

pub(crate) struct TaskManager {
    queued  : VecDeque<Arc<Mutex<Box<dyn Task>>>>,
    running : Arc<Mutex<HashMap<TaskId, Arc<Mutex<Box<dyn Task>>>>>>,

    canceling: AtomicBool,
}

/** Maximum number of active tasks. */
const MAX_ACTIVE_TASKS: usize = 8;

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self {
            queued  : VecDeque::new(),
            running : Arc::new(Mutex::new(HashMap::new())),
            canceling: AtomicBool::new(false),
        }
    }

    pub(crate) async fn add(&mut self, task: Arc<Mutex<Box<dyn Task>>>) {
        self.add_prior(task, false).await
    }

    pub(crate) async fn add_prior(
        &mut self,
        task: Arc<Mutex<Box<dyn Task>>>,
        prior: bool
    ) {
        if self.canceling.load(Ordering::SeqCst) {
            return;
        }
        if task.lock().unwrap().is_ended() {
            return;
        }

        let cloned_task = task.clone();
        let running = self.running.clone();
        task.lock().unwrap().with_end_handler({
            Consumer::new(move |_| {
                let taskid = cloned_task.lock().unwrap().task_id();
                running.lock().unwrap().remove(&taskid);
            })
        });

        if task.lock().unwrap().state() == State::Running {
            let taskid = task.lock().unwrap().task_id();
            self.running.lock().unwrap().insert(taskid, task);
            return;
        }

        if !task.lock().unwrap().set_state_if(&State::Initialized, State::Queued) {
            error!("!!!INTERNAL ERROR: task is not in INITIAL state: {}", task.lock().unwrap());
			//task.endHandler(null);
			return;
        }

        match prior {
            true => self.queued.push_front(task),
            false => self.queued.push_back(task),
        };

        self.dequeue().await;
    }

    fn is_ready(&self) -> bool {
        !self.canceling.load(Ordering::SeqCst) &&
            self.running.lock().unwrap().len() < MAX_ACTIVE_TASKS
    }

    pub(crate) async fn dequeue(&mut self) {
        while self.is_ready() {
            let Some(task) = self.queued.pop_front() else {
                debug!("Queue drained.");
                break;
            };

            if task.lock().unwrap().is_ended() {
                continue;
            }

            debug!("Start task: {}", task.lock().unwrap());

            let taskid = task.lock().unwrap().task_id();
            self.running.lock().unwrap().insert(taskid, task.clone());

            let _ = task::spawn({
                let task = task.clone();
                async move {
                    task.lock().unwrap().start();
                }
            }).await.unwrap();
        }
    }

    pub(crate) fn cancel_all(&mut self) {
        self.canceling.store(true, Ordering::SeqCst);

        for (_, t) in self.running.lock().unwrap().drain() {
            t.lock().unwrap().cancel();
        }
        for t in self.queued.drain(..) {
            t.lock().unwrap().cancel();
        }

        self.running.lock().unwrap().clear();
        self.queued.clear();

        self.canceling.store(false, Ordering::SeqCst);
    }
}
