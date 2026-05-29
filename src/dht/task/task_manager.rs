use std::{
    sync::{Arc, Mutex},
    sync::atomic::{AtomicBool, Ordering},
    collections::{VecDeque, HashMap},
};
use tokio::task;
use log::{debug, error};

use crate::locked;
use crate::dht::{
    consumer::Consumer,
    task::{Task, task::{State, TaskId}}
};

const MAX_ACTIVE_TASKS: usize = 8;

pub(crate) struct TaskManager {
    queued      : VecDeque<Arc<Mutex<Box<dyn Task>>>>,
    running     : Arc<Mutex<HashMap<TaskId, Arc<Mutex<Box<dyn Task>>>>>>,
    canceling   : AtomicBool,
}

impl TaskManager {
    pub(crate) fn new_shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            queued  : VecDeque::new(),
            running : Arc::new(Mutex::new(HashMap::new())),
            canceling: AtomicBool::new(false),
        }))
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

        let mut locked  = task.lock().unwrap();
        let task = task.clone();
        if locked.is_ended() {
            return;
        }

        let taskid = locked.task_id();
        let running = self.running.clone();
        locked.with_ended_handler(
            Consumer::new(move |_| {
                locked!(running).remove(&taskid);
            })
        );

        if locked.task_state() == State::Running {
            //self.running.insert(taskid, task);
            return;
        }

        if !locked.set_state_if(&State::Initialized, State::Queued) {
            error!("!Panic: task is not in Initialized state: {}", locked);
			//TODO: locked.ended_handler(());
			return;
        }
        drop(locked);

        match prior {
            true => self.queued.push_front(task),
            false => self.queued.push_back(task),
        };
        self.dequeue().await;
    }

    #[inline(always)]
    fn is_ready(&self) -> bool {
        !self.canceling.load(Ordering::SeqCst) &&
            locked!(self.running).len() < MAX_ACTIVE_TASKS
    }

    pub(crate) async fn dequeue(&mut self) {
        while self.is_ready() {
            let Some(task) = self.queued.pop_front() else {
                debug!("Queue drained.");
                break;
            };

            let locked = locked!(task);
            let task = task.clone();
            if locked.is_ended() {
                drop(locked);
                continue;
            }

            debug!("Start task: {}", locked!(task));

            let taskid = locked.task_id();
            locked!(self.running).insert(taskid, task.clone());
            drop(locked);

            let _ = task::spawn({
                let task = task.clone();
                async move {
                    task.lock().unwrap().start();
                }
            }).await;
        }
    }

    pub(crate) fn cancel_all(&mut self) {
        self.canceling.store(true, Ordering::SeqCst);

        for (_, t) in locked!(self.running).drain() {
            t.lock().unwrap().cancel();
        }
        for t in self.queued.drain(..) {
            t.lock().unwrap().cancel();
        }

        self.canceling.store(false, Ordering::SeqCst);
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        locked!(self.running).clear();
        self.queued.clear();
    }
}
