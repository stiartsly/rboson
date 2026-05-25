use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{sync::{mpsc, oneshot}};
use tokio_util::time::{DelayQueue};

use crate::{
    core::errors::{Result, StateError},
};

type JobFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type JobHandler = Arc<dyn Fn() -> JobFuture + Send + Sync>;

pub(crate) type TaskHandle = u64;

#[derive(Clone)]
pub(crate) struct Job {
    pub(crate) id: u64,
    pub(crate) interval: Option<Duration>,
    cb: JobHandler,
    active: Arc<AtomicBool>,
}

impl Job {
    fn new(id: u64, interval: Option<Duration>, callback: JobHandler) -> Self {
        Self {
            id,
            interval,
            cb: callback,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn invoke(&self) -> JobFuture {
        (self.cb)()
    }

    pub(crate) fn cancel(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

pub(crate) enum Command {
    Add {
        delay: Duration,
        job: Job,
    },
    Remove {
        job_id: u64,
        reply: oneshot::Sender<bool>,
    },
    Stop {
        reply: oneshot::Sender<()>,
    },
}

pub(crate) struct TimerQ {
    rx: mpsc::UnboundedReceiver<Command>,
    queue: DelayQueue<Job>,
}

pub(crate) struct Client {
    next_jobid: AtomicU64,
    tx: mpsc::UnboundedSender<Command>,
}

impl Client {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            next_jobid: AtomicU64::new(1),
            tx,
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
        let taskid = self.next_jobid.fetch_add(1, Ordering::Relaxed);
        let cb: JobHandler = Arc::new(move || Box::pin(cb()));
        let job = Job::new(taskid, interval, cb);

        self.tx
            .send(Command::Add { delay, job })
            .map_err(|_| StateError::new(format!("Error: channel closed")))?;

        Ok(taskid)
    }

    pub(crate) async fn cancel(&self, handle: TaskHandle) -> Result<bool> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Remove {
                job_id: handle,
                reply: reply_tx,
            })
            .map_err(|_| StateError::new(format!("Error: channel closed")))?;

        Ok(reply_rx.await
            .map_err(|_| StateError::new(format!("Error: channel closed")))?)
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Stop { reply: reply_tx })
            .map_err(|_| StateError::new(format!("Error: channel closed")))?;

        Ok(reply_rx.await
            .map_err(|_| StateError::new(format!("Error: channel closed")))?)
    }
}
