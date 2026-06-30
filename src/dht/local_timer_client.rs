use std::{
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
};
use tokio::{sync::{
    mpsc,
    oneshot,
}};

use crate::errors::{Result, StateError};
use super::handler::LocalAsyncHandler as AsyncHandler;

pub(crate) enum TimerCmd {
    Add {
        timer_id: u64,
        delay: u64,
        interval: Option<u64>,
        cb: AsyncHandler<()>,
    },
    Cancel {
        timer_id: u64,
    },
    Stop {
        complete: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub(crate) struct TimerClient {
    sender: mpsc::UnboundedSender<TimerCmd>,
    next_id: Arc<AtomicU64>,
}

impl TimerClient {
    pub(crate) fn new(
        sender: mpsc::UnboundedSender<TimerCmd>,
    ) -> Self {
        Self {
            sender,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_timer_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn add_timer(
        &self,
        delay: u64,
        interval: Option<u64>,
        callback: AsyncHandler<()>,
    ) -> Result<u64>
    {
        let timer_id = self.next_timer_id();
        self.sender.send(
            TimerCmd::Add {
                timer_id,
                delay,
                interval,
                cb: callback,
            }
        ).map_err(|_| {
            StateError::new("timer queue channel closed")
        }).map(|_| timer_id)
    }

    pub(crate) fn cancel_timer(
        &self,
        timer_id: u64,
    ) -> Result<()> {
        self.sender.send(
            TimerCmd::Cancel {timer_id}
        ).map_err(|_| {
            StateError::new("timer queue channel closed")
        }).map(|_| ())
    }

    pub(crate) async fn stop(
        &self,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();

        self.sender.send(
            TimerCmd::Stop {complete: tx}
        ).map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        rx.await.map_err(|_| {
            StateError::new("timer queue shutdown acknowledgement dropped")
        })?;
        Ok(())
    }
}