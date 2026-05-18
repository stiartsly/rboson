use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use futures::StreamExt;
use tokio::{
    sync::{mpsc, oneshot},
    task,
    task::JoinHandle,
};
use tokio_util::time::{delay_queue::Key, DelayQueue};


use crate::{
    core::errors::Result,
    dht::scheduler_error::SchedulerError,
};

type TaskFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type TaskCallback = Arc<dyn Fn() -> TaskFuture + Send + Sync>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TaskHandle {
    id: u64,
}

impl TaskHandle {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Clone)]
struct Job {
    id: u64,
    interval: Option<Duration>,
    cb: TaskCallback,
    active: Arc<AtomicBool>,
}

impl Job {
    fn new(id: u64, interval: Option<Duration>, callback: TaskCallback) -> Self {
        Self {
            id,
            interval,
            cb: callback,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    fn invoke(&self) -> TaskFuture {
        (self.cb)()
    }

    fn cancel(&self) {
        self.active.store(false, Ordering::Release);
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

enum Command {
    Add {
        delay: Duration,
        job: Job,
    },
    Remove {
        task_id: u64,
        reply: oneshot::Sender<bool>,
    },
    Stop {
        reply: oneshot::Sender<()>,
    },
}

pub(crate) struct Scheduler {
    next_taskid: AtomicU64,
    tx: mpsc::UnboundedSender<Command>,
    runner: JoinHandle<()>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let runner = task::spawn(async move {
            Self::run_loop(rx).await;
        });

        Self {
            next_taskid: AtomicU64::new(1),
            tx,
            runner,
        }
    }

    pub(crate) fn add<F, Fut>(
        &self,
        delay: Duration,
        interval: Option<Duration>,
        cb: F,
    ) -> Result<TaskHandle>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let taskid = self.next_taskid.fetch_add(1, Ordering::Relaxed);
        let cb: TaskCallback = Arc::new(move || Box::pin(cb()));
        let job = Job::new(taskid, interval, cb);

        self.tx
            .send(Command::Add { delay, job })
            .map_err(|_| SchedulerError::new(format!("Scheduler closed")))?;

        Ok(TaskHandle { id: taskid })
    }

    pub(crate) async fn cancel(&self, handle: TaskHandle) -> Result<bool> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Remove {
                task_id: handle.id,
                reply: reply_tx,
            })
            .map_err(|_| SchedulerError::new(format!("Scheduler closed")))?;

        Ok(reply_rx.await.map_err(|_| SchedulerError::new(format!("Scheduler closed")))?)
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Stop { reply: reply_tx })
            .map_err(|_| SchedulerError::new(format!("Scheduler closed")))?;

        Ok(reply_rx.await.map_err(|_| SchedulerError::new(format!("Scheduler closed")))?)
    }

    async fn run_loop(mut rx: mpsc::UnboundedReceiver<Command>) {
        let mut queue = DelayQueue::new();
        let mut jobs = HashMap::<u64, Job>::new();
        let mut keys = HashMap::<u64, Key>::new();

        loop {
            tokio::select! {
                biased;

                command = rx.recv() => {
                    match command {
                        Some(Command::Add { delay, job }) => {
                            let task_id = job.id;
                            if let Some(key) = keys.remove(&task_id) {
                                let _ = queue.remove(&key);
                            }

                            let key = queue.insert(task_id, delay);
                            keys.insert(task_id, key);
                            jobs.insert(task_id, job);
                        }
                        Some(Command::Remove { task_id, reply }) => {
                            let removed = Self::remove_task(task_id, &mut queue, &mut jobs, &mut keys);
                            let _ = reply.send(removed);
                        }
                        Some(Command::Stop { reply }) => {
                            jobs.clear();
                            keys.clear();
                            queue.clear();
                            let _ = reply.send(());
                            break;
                        }
                        None => {
                            if queue.is_empty() {
                                break;
                            }
                        }
                    }
                }
                maybe_expired = queue.next(), if !queue.is_empty() => {
                    let Some(expired) = maybe_expired else {
                        continue;
                    };

                    let task_id = expired.into_inner();
                    keys.remove(&task_id);

                    let Some(task) = jobs.get(&task_id).cloned() else {
                        continue;
                    };

                    if !task.is_active() {
                        jobs.remove(&task_id);
                        continue;
                    }

                    if let Some(interval) = task.interval {
                        if task.is_active() {
                            let key = queue.insert(task_id, interval);
                            keys.insert(task_id, key);
                        }
                    } else {
                        jobs.remove(&task_id);
                    }

                    tokio::spawn(async move {
                        if task.is_active() {
                            task.invoke().await;
                        }
                    });
                }
                else => break,
            }
        }
    }

    fn remove_task(
        task_id: u64,
        queue: &mut DelayQueue<u64>,
        jobs: &mut HashMap<u64, Job>,
        keys: &mut HashMap<u64, Key>,
    ) -> bool {
        let mut removed = false;

        if let Some(job) = jobs.remove(&task_id) {
            job.cancel();
            removed = true;
        }

        if let Some(key) = keys.remove(&task_id) {
            let _ = queue.remove(&key);
            removed = true;
        }

        removed
    }
}