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

use super::timer_queue::{
    Command,
    TimerId,
    Timer,
};

#[derive(Clone)]
pub(crate) struct TimerClient {
    sender: mpsc::Sender<Command>,
    next_id: Arc<AtomicU64>,
}

impl TimerClient {
    pub(crate) fn new(
        sender: mpsc::Sender<Command>,
    ) -> Self {
        Self {
            sender,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_timer_id(&self) -> TimerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) async fn add_timer<F>(
        &self,
        delay: u64,
        interval: Option<u64>,
        callback: F,
    ) -> Result<TimerId>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let id = self.next_timer_id();
        let delay = Duration::from_millis(delay);
        let interval = interval.map(Duration::from_millis);
        self.sender.send(
            Command::Add {
                delay,
                timer: Timer::new(id, interval, Arc::new(callback)),
            }
        ).await.map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        Ok(id)
    }

    #[allow(unused)]
    pub(crate) async fn cancel_timer(
        &self,
        id: TimerId,
    ) -> Result<()> {
        self.sender.send(
            Command::Cancel { id }
        ).await.map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        Ok(())
    }

    pub(crate) async fn stop_all(
        &self,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();

        self.sender.send(
            Command::Stop {
                complete: tx,
            }
        ).await.map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        rx.await.map_err(|_| {
            StateError::new("timer queue shutdown acknowledgement dropped")
        })?;
        Ok(())
    }
}