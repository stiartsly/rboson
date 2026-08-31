use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::mpsc;

use crate::errors::{Result, StateError};
use super::handler::LocalHandler;

pub(crate) type TimerId = u64;

pub(crate) enum GenericTimerCmd<H> {
    Add {
        timer_id: TimerId,
        delay: u64,
        interval: Option<u64>,
        cb: H,
    },
}

#[derive(Clone)]
pub(crate) struct GenericTimerClient<H> {
    sender: mpsc::UnboundedSender<GenericTimerCmd<H>>,
    next_id: Arc<AtomicU64>,
}

impl<H> GenericTimerClient<H> {
    pub(crate) fn new(
        sender: mpsc::UnboundedSender<GenericTimerCmd<H>>,
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
            GenericTimerCmd::Add {
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
/*
    pub(crate) fn cancel_timer(
        &self,
        timer_id: TimerId,
    ) -> Result<()> {
        self.sender.send(
            GenericTimerCmd::Cancel { timer_id }
        ).map_err(|_| {
            StateError::new("timer channel closed")
        }).map(|_| ())
    }

    pub(crate) async fn stop(
        &self,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(
            GenericTimerCmd::Stop { complete: tx }
        ).map_err(|_| {
            StateError::new("timer channel closed")
        })?;

        rx.await.map_err(|_| {
            StateError::new("timer shutdown acknowledgement dropped")
        }).map(|_| ())
    }
    */
}

pub(crate) type TimerCmd = GenericTimerCmd<LocalHandler<()>>;
pub(crate) type TimerClient = GenericTimerClient<LocalHandler<()>>;
