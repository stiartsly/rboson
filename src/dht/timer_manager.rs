use std::{
    collections::HashMap,
    time::Duration,
    sync::atomic::AtomicU64,
};
use futures::StreamExt;
use tokio_util::time::{
    delay_queue::Key,
    DelayQueue,
};
use crate::dht::handler::{AsyncHandler, LocalHandler, Callable};

pub(crate) type TimerId = u64;

struct GenericTimerEntry<H> {
    interval    : Option<Duration>,
    handler     : H,
    key         : Key,
}

pub struct GenericTimerManager<H> {
    next_id     : AtomicU64,
    delay_queue : DelayQueue<TimerId>,
    timers      : HashMap<TimerId, GenericTimerEntry<H>>,
}

impl<H: Callable<()>> GenericTimerManager<H> {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            delay_queue: DelayQueue::new(),
            timers: HashMap::new(),
        }
    }

    fn _add_timer(&mut self,
        id      : TimerId,
        delay   : Duration,
        interval: Option<Duration>,
        handler : H
    ) {
        if let Some(existing) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&existing.key);
        }

        let key = self.delay_queue.insert(id, delay);
        let entry = GenericTimerEntry { handler, interval, key };
        self.timers.insert(id, entry);
    }

    pub(crate) fn add_timer(&mut self,
        id      : TimerId,
        delay   : u64,
        interval: Option<u64>,
        cb      : H
    ) {
        let delay = Duration::from_millis(delay);
        let interval = interval.map(Duration::from_millis);
        self._add_timer(id, delay, interval, cb);
    }

    pub(crate) fn cancel_timer(&mut self, id: TimerId) {
        if let Some(entry) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&entry.key);
        }
    }

    pub(crate) fn stop_all(&mut self) {
        self.delay_queue.clear();
        self.timers.clear();
    }

    pub(crate) async fn fire_expired(&mut self, id: TimerId) {
        let Some(entry) = self.timers.remove(&id) else {
            return;
        };

        entry.handler.call_boxed(()).await;

        if let Some(interval) = entry.interval {
            self._add_timer(id, interval, Some(interval), entry.handler);
        }
    }

    pub(crate) async fn next_expired(&mut self) -> Option<TimerId> {
        let expired = self.delay_queue.next().await;
        expired.map(|e| e.into_inner())
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.timers.is_empty()
    }
}

// Alias for standard (thread-safe Send) timer manager
pub(crate) type AsyncTimerManager = GenericTimerManager<AsyncHandler<()>>;

// Alias for local (not Send) timer manager
pub(crate) type LocalTimerManager = GenericTimerManager<LocalHandler<()>>;
