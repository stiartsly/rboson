use std::{
    collections::HashMap,
    time::Duration,
    sync::atomic::{AtomicU64, Ordering},
};
use futures::StreamExt;
use tokio_util::time::{
    delay_queue::Key,
    DelayQueue,
};

use crate::dht::consumer::AsyncConsumer;
pub(crate) type TimerId = u64;

struct TimerEntry<T> {
    interval    : Option<Duration>,
    cb          : AsyncConsumer<()>,
    key         : Key,
}

pub struct TimerManager {
    next_id     : AtomicU64,
    delay_queue : DelayQueue<TimerId>,
    timers      : HashMap<TimerId, TimerEntry>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            delay_queue: DelayQueue::new(),
            timers: HashMap::new(),
        }
    }

    fn next_timer_id(&self) -> TimerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn _add_timer(&mut self,
        delay   : Duration,
        interval: Option<Duration>,
        cb      : AsyncConsumer<()>
    ) {
        let id = self.next_timer_id();

        if let Some(existing) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&existing.key);
        }

        let key = self.delay_queue.insert(id, delay);
        let entry = TimerEntry {
            cb: cb,
            interval: interval,
            key,
        };
        self.timers.insert(id, entry);
    }

    fn add_timer(&mut self,
        delay   : u64,
        interval: Option<u64>,
        cb      : AsyncConsumer<()>
    ) {
        let delay = Duration::from_millis(delay);
        let interval = interval.map(Duration::from_millis);
        self._add_timer(delay, interval, cb);
    }

    fn cancel_timer(&mut self, id: TimerId) {
        if let Some(entry) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&entry.key);
        }
    }

    fn stop_all(&mut self) {
        self.delay_queue.clear();
        self.timers.clear();
    }

    async fn fire_expired(&mut self, id: TimerId) {
        let Some(entry) = self.timers.remove(&id) else {
            return;
        };

        let cb = entry.cb;
        cb.accept(()).await;

        if let Some(interval) = entry.interval {
            self._add_timer(interval, Some(interval), cb);
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
