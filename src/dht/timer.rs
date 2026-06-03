use std::{
    time::Duration,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    }
};
use tokio::{sync::{mpsc, oneshot}};
use tokio_util::time::{DelayQueue};
use log::error;

use crate::{
    core::errors::{Result, StateError},
};

pub(crate) type TimerId = u64;

#[derive(Clone)]
pub(crate) struct Job {
    pub(crate) id: TimerId,
    pub(crate) interval: Option<Duration>,

    cb: Arc<Box<dyn Fn() + Send + Sync>>,
    active: Arc<AtomicBool>,
}

impl Job {
    fn new<F>(id: TimerId, interval: Option<Duration>, cb: F) -> Self
    where F: Fn() + Send + Sync +'static
    {
        Self {
            id,
            interval,
            cb: Arc::new(Box::new(cb)),
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn invoke(&self) {
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
        job_id: TimerId,
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
    pub(crate) fn new(tx: mpsc::UnboundedSender<Command>) -> Self {
        Self {
            next_jobid: AtomicU64::new(1),
            tx,
        }
    }

    pub(crate) fn add_timer<F>(
        &self,
        delay: Duration,
        interval: Option<Duration>,
        cb: F,
    ) -> Result<&Self>
    where
        F: Fn() + Send + Sync + 'static
    {
        let taskid = self.next_jobid.fetch_add(1, Ordering::Relaxed);
        let job = Job::new(taskid, interval, cb);

        self.tx
            .send(Command::Add { delay, job })
            .map_err(|_| StateError::new(format!("Error: channel closed")))?;

        Ok(self)
    }

    pub(crate) fn add_timer_if(
        &self,
        predicate: bool,
        delay: Duration,
        interval: Option<Duration>,
        cb: impl Fn() + Send + Sync + 'static,
    ) -> Result<&Self> {
        if predicate {
            return self.add_timer(delay, interval, cb);
        } else {
            Ok(self)
        }
    }

    #[allow(unused)]
    pub(crate) async fn cancel_timer(&self, handle: TimerId) -> Result<bool> {
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

    pub(crate) async fn stop(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::Stop { reply: reply_tx })
            .map_err(|_| error!("Error: channel closed"));

        let _ = reply_rx.await
            .map_err(|_| error!("Error: channel closed"));
    }
}
