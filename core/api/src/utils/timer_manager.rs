use futures::StreamExt;
use std::{collections::HashMap, time::Duration};
use tokio_util::time::{delay_queue::Key, DelayQueue};

use super::handler::{BoxHandler, Callable, LocalBoxHandler};

pub type TimerId = u64;

struct TimerEntry<H> {
    interval: Option<Duration>,
    handler: H,
    key: Key,
}

pub struct TimerManager<H> {
    delay_queue: DelayQueue<TimerId>,
    timers: HashMap<TimerId, TimerEntry<H>>,
}

impl<H: Callable<()>> TimerManager<H> {
    pub fn new() -> Self {
        Self {
            delay_queue: DelayQueue::new(),
            timers: HashMap::new(),
        }
    }

    fn _add_timer(&mut self, id: TimerId, delay: Duration, interval: Option<Duration>, handler: H) {
        if let Some(existing) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&existing.key);
        }

        let key = self.delay_queue.insert(id, delay);
        let entry = TimerEntry {
            handler,
            interval,
            key,
        };
        self.timers.insert(id, entry);
    }

    pub fn add_timer(&mut self, id: TimerId, delay: u64, interval: Option<u64>, cb: H) {
        let delay = Duration::from_millis(delay);
        let interval = interval.map(Duration::from_millis);
        self._add_timer(id, delay, interval, cb);
    }

    pub fn cancel_timer(&mut self, id: TimerId) {
        if let Some(entry) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&entry.key);
        }
    }

    pub fn stop_all(&mut self) {
        self.delay_queue.clear();
        self.timers.clear();
    }

    pub async fn fire_expired(&mut self, id: TimerId) {
        let Some(entry) = self.timers.remove(&id) else {
            return;
        };

        entry.handler.call_boxed(()).await;

        if let Some(interval) = entry.interval {
            self._add_timer(id, interval, Some(interval), entry.handler);
        }
    }

    pub async fn next_expired(&mut self) -> Option<TimerId> {
        let expired = self.delay_queue.next().await;
        expired.map(|e| e.into_inner())
    }

    pub fn is_idle(&self) -> bool {
        self.timers.is_empty()
    }
}

// Alias for standard (thread-safe Send) timer manager
pub type BoxTimerManager = TimerManager<BoxHandler<()>>;

// Alias for local (not Send) timer manager
pub type LocalBoxTimerManager = TimerManager<LocalBoxHandler<()>>;
