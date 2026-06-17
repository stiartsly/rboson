use std::{
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    time::Duration,
};
use tokio::{sync::{
    mpsc,
    oneshot,
}};

use crate::errors::{Result, StateError};

use super::consumer::AsyncConsumer;
use super::timer_queue::{
    Command,
    TimerId,
    Timer,
};

#[derive(Clone)]
pub(crate) struct TimerClient {
    sender: mpsc::UnboundedSender<Command>,
    next_id: Arc<AtomicU64>,
}

impl TimerClient {
    pub(crate) fn new(
        sender: mpsc::UnboundedSender<Command>,
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
        callback: AsyncConsumer<()>,
    ) -> Result<TimerId>
    {
        let id = self.next_timer_id();
        let delay = Duration::from_millis(delay);
        let interval = interval.map(Duration::from_millis);
        self.sender.send(
            Command::Add {
                delay,
                timer: Timer::new(id, interval, callback),
            }
        ).map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        Ok(id)
    }

    pub(crate) fn cancel_timer(
        &self,
        timer_id: TimerId,
    ) -> Result<()> {
        self.sender.send(
            Command::Cancel {
                id: timer_id,
            }
        ).map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        Ok(())
    }

    pub(crate) async fn stop(
        &self,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();

        self.sender.send(
            Command::Stop {
                complete: tx,
            }
        ).map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        rx.await.map_err(|_| {
            StateError::new("timer queue shutdown acknowledgement dropped")
        })?;
        Ok(())
    }
}