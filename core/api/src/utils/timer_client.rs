use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot};

use super::handler::{BoxHandler, LocalBoxHandler};
use crate::errors::{Result, StateError};

pub type TimerId = u64;

pub enum TimerCmd<H> {
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
pub struct TimerClient<H> {
    sender: mpsc::UnboundedSender<TimerCmd<H>>,
    next_id: Arc<AtomicU64>,
}

impl<H> TimerClient<H> {
    pub fn new(sender: mpsc::UnboundedSender<TimerCmd<H>>) -> Self {
        Self {
            sender,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_timer_id(&self) -> TimerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn add_timer(
        &self,
        delay: u64,
        interval: Option<u64>,
        callback: H,
    ) -> Result<TimerId> {
        let timer_id = self.next_timer_id();
        self.sender
            .send(TimerCmd::Add {
                timer_id,
                delay,
                interval,
                cb: callback,
            })
            .map_err(|_| StateError::new("timer channel closed"))?;
        Ok(timer_id)
    }

    pub fn cancel_timer(&self, timer_id: TimerId) -> Result<()> {
        self.sender
            .send(TimerCmd::Cancel { timer_id })
            .map_err(|_| StateError::new("timer channel closed"))?;
        Ok(())
    }

    pub async fn stop_timers(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(TimerCmd::Stop { complete: tx })
            .map_err(|_| StateError::new("timer channel closed"))?;

        rx.await
            .map_err(|_| StateError::new("timer shutdown acknowledgement dropped"))?;
        Ok(())
    }
}

// Aliases for standard (thread-safe Send) timer client
pub type BoxTimerCmd = TimerCmd<BoxHandler<()>>;
pub type BoxTimerClient = TimerClient<BoxHandler<()>>;

// Aliases for local (not Send) timer client
pub type LocalBoxTimerCmd = TimerCmd<LocalBoxHandler<()>>;
pub type LocalBoxTimerClient = TimerClient<LocalBoxHandler<()>>;
