use std::{
    fmt,
    collections::HashMap,
    time::Duration,
    sync::{Arc, Mutex}
};
use futures::StreamExt;
use tokio::{
    sync::{mpsc, oneshot},
};
use tokio_util::time::{
    delay_queue::Key,
    DelayQueue,
};

use crate::dht::consumer::AsyncConsumer;
pub type TimerId = u64;

pub(crate) struct Timer {
    id: TimerId,
    pub(crate) interval: Option<Duration>,
    cb: AsyncConsumer<()>,
}

impl Timer {
    pub(crate) fn new(id: TimerId, interval: Option<Duration>, cb: AsyncConsumer<()>) -> Self {
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
    Stop {
        complete: oneshot::Sender<()>,
    },
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Add { delay, timer } => write!(f, "Add Timer {{ id: {}, delay: {:?} }}", timer.id, delay),
            Command::Cancel { id } => write!(f, "Cancel Timer {{ id: {} }}", id),
            Command::Stop { .. } => write!(f, "Stop timer task"),
        }
    }
}

struct TimerEntry {
    interval: Option<Duration>,
    callback: AsyncConsumer<()>,
    key: Key,
}

pub struct TimerQueue {
    receiver    : mpsc::UnboundedReceiver<Command>,
    delay_queue : DelayQueue<TimerId>,
    timers      : HashMap<TimerId, TimerEntry>,
}

impl TimerQueue {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<Command>,
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
            callback: timer.cb,
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

    async fn fire_expired(&mut self, id: TimerId) {
        let Some(entry) = self.timers.remove(&id) else {
            return;
        };

        let cb = entry.callback;
        cb.accept(()).await;

        if let Some(interval) = entry.interval {
            let timer = Timer::new(id, Some(interval), cb);
            self.add_timer(interval, timer);
        }
    }

    pub(crate) async fn run(mut self, quit: Arc<Mutex<bool>>) {
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
                        Command::Stop { complete } => {
                            self.stop_all();
                            let _ = complete.send(());
                            if *quit.lock().unwrap() {
                                break;
                            }
                        }
                    }
                }
                Some(expired) = self.delay_queue.next(), if !self.is_idle() => {
                    if *quit.lock().unwrap() {
                        break;
                    } else {
                        self.fire_expired(expired.into_inner()).await;
                    }
                }
            }
        }
    }
}
