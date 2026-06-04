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
    TimerCallback,
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
        delay: Duration,
        interval: Option<Duration>,
        callback: F,
    ) -> Result<TimerId>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let id = self.next_timer_id();
        let cb: TimerCallback = Arc::new(callback);

        self.sender.send(
            Command::Add {
                delay,
                timer: Timer::new(id, interval, cb),
            }
        ).await.map_err(|_| {
            StateError::new("timer queue channel closed")
        })?;
        Ok(id)
    }

    pub(crate) async fn add_timer_if<F>(
        &self,
        predicate: bool,
        delay: Duration,
        interval: Option<Duration>,
        callback: F,
    ) -> Result<Option<TimerId>>
    where
        F: Fn() + Send + Sync + 'static,
    {
        if predicate {
            self.add_timer(delay, interval, callback).await.map(Some)
        } else {
            Ok(None)
        }
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
            Command::StopAll {
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