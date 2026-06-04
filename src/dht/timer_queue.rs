use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;
use tokio::{
    sync::{mpsc, oneshot},
};
use tokio_util::time::{
    delay_queue::Key,
    DelayQueue,
};

pub type TimerId = u64;
pub type TimerCallback =
    Arc<dyn Fn() + Send + Sync + 'static>;

pub(crate) struct Timer {
    id: TimerId,
    pub(crate) interval: Option<Duration>,
    cb: TimerCallback,
}

impl Timer {
    pub(crate) fn new(id: TimerId, interval: Option<Duration>, cb: TimerCallback) -> Self {
        Self {
            id,
            interval,
            cb,
        }
    }
}

pub enum Command {
    Add {
        delay: Duration,
        timer: Timer,
    },
    Cancel {
        id: TimerId,
    },
    StopAll {
        complete: oneshot::Sender<()>,
    },
}

struct TimerEntry {
    interval: Option<Duration>,
    callback: TimerCallback,
    key: Key,
}

pub struct TimerQueue {
    receiver    : mpsc::Receiver<Command>,
    delay_queue : DelayQueue<TimerId>,
    timers      : HashMap<TimerId, TimerEntry>,
}

impl TimerQueue {
    pub fn new(
        receiver: mpsc::Receiver<Command>,
    ) -> Self {
        Self {
            receiver,
            delay_queue: DelayQueue::new(),
            timers: HashMap::new(),
        }
    }

    fn add_timer(&mut self, delay: Duration, timer: Timer) {
        let id = timer.id;
        if let Some(existing) = self.timers.remove(&id) {
            let _ = self.delay_queue.remove(&existing.key);
        }

        let key = self.delay_queue.insert(id, delay);
        let entry = TimerEntry {
            callback: timer.cb.clone(),
            interval: timer.interval,
            key,
        };
        self.timers.insert(id, entry);
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

    fn is_idle(&self) -> bool {
        self.timers.is_empty()
    }

    fn fire_expired(&mut self, id: TimerId) {
        let Some(entry) = self.timers.remove(&id) else {
            return;
        };

        (entry.callback)();

        if let Some(interval) = entry.interval {
            let timer = Timer::new(id, Some(interval), entry.callback.clone());
            self.add_timer(interval, timer);
        }
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.receiver.recv() => {
                    match cmd {
                        Command::Add { delay, timer } => {
                            self.add_timer(delay, timer);
                        }
                        Command::Cancel { id } => {
                            self.cancel_timer(id);
                        }
                        Command::StopAll { complete } => {
                            self.stop_all();
                            let _ = complete.send(());
                            break;
                        }
                    }
                }

                Some(expired) = self.delay_queue.next(), if !self.is_idle() => {
                    self.fire_expired(expired.into_inner());
                }
                else => {}
            }
        }
    }
}