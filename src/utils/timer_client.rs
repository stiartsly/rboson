use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering}
};
use tokio::sync::{mpsc, oneshot};

use crate::errors::{Result, StateError};
use super::handler::{BoxHandler, LocalBoxHandler};

pub(crate) type TimerId = u64;

pub(crate) enum TimerCmd<H> {
    Add {
        timer_id: TimerId,
        delay: u64,
        interval: Option<u64>,
        cb: H,
    },
    Cancel {
        timer_id: TimerId,
    },
    Stop {
        complete: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub(crate) struct TimerClient<H> {
    sender: mpsc::UnboundedSender<TimerCmd<H>>,
    next_id: Arc<AtomicU64>,
}

impl<H> TimerClient<H> {
    pub(crate) fn new(
        sender: mpsc::UnboundedSender<TimerCmd<H>>,
    ) -> Self {
        Self {
            sender,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_timer_id(&self) -> TimerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn add_timer(
        &self,
        delay: u64,
        interval: Option<u64>,
        callback: H,
    ) -> Result<TimerId> {
        let timer_id = self.next_timer_id();
        self.sender.send(
            TimerCmd::Add {
                timer_id,
                delay,
                interval,
                cb: callback,
            }
        ).map_err(|_| {
            StateError::new("timer channel closed")
        }).map(|_|
            timer_id
        )
    }

    pub(crate) fn cancel_timer(
        &self,
        timer_id: TimerId,
    ) -> Result<()> {
        self.sender.send(
            TimerCmd::Cancel { timer_id }
        ).map_err(|_| {
            StateError::new("timer channel closed")
        }).map(|_| ())
    }

    pub(crate) async fn stop_timers(
        &self,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(
            TimerCmd::Stop { complete: tx }
        ).map_err(|_| {
            StateError::new("timer channel closed")
        })?;

        rx.await.map_err(|_| {
            StateError::new("timer shutdown acknowledgement dropped")
        }).map(|_| ())
    }
}

// Aliases for standard (thread-safe Send) timer client
pub(crate) type BoxTimerCmd = TimerCmd<BoxHandler<()>>;
pub(crate) type BoxTimerClient = TimerClient<BoxHandler<()>>;

// Aliases for local (not Send) timer client
pub(crate) type LocalBoxTimerCmd = TimerCmd<LocalBoxHandler<()>>;
pub(crate) type LocalBoxTimerClient = TimerClient<LocalBoxHandler<()>>;
