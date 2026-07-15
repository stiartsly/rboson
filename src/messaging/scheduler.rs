use std::sync::{Mutex, Arc};
use std::collections::LinkedList;
use std::pin::Pin;
use futures::future::Future;
use rbtree::RBTree;
use tokio::time::{Duration, Instant};
use crate::lock;

type JobFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;
type JobCallback = Box<dyn FnMut() -> JobFuture + 'static>;

struct Job {
    cb: JobCallback,
    interval: Option<Duration>,
}

impl Job {
    fn new(cb: JobCallback, input_interval: u64 /* ms */) -> Self {
        let mut interval = None;
        if input_interval > 0 {
            let d = Duration::from_millis(input_interval);
            interval = Some(d);
        }

        Self {
            cb,
            interval,
        }
    }

    pub(crate) async fn invoke(&mut self) {
        (self.cb)().await
    }
}

pub(crate) struct Scheduler {
    updated: bool,  // job has been added or popped recently.
    now: Instant,   // to retify current time.

    timers: RBTree<Instant, LinkedList<Box<Job>>>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Scheduler {
            updated: false,
            now: Instant::now(),
            timers: RBTree::new(),
        }
    }

    // add oneshot job.
    pub(crate) fn add_oneshot(&mut self, cb: JobCallback, start: u64){
        self.add_job(
            Duration::from_millis(start),
            Box::new(Job::new(cb, 0))
        );
    }

    // add periodic job with specific interval.
    pub(crate) fn add(&mut self, cb: JobCallback, start: u64, interval: u64) {
        self.add_job(
            Duration::from_millis(start),
            Box::new(Job::new(cb, interval)),
        );
    }

    pub(crate) fn is_updated(&self) -> bool {
        self.updated
    }

    pub(crate) fn next_timeout(&self) -> Instant {
        self.timers.iter().next().map_or(
            self.now + Duration::from_secs(3600), // 60*60
            |timer| timer.0.clone()
        )
    }

    fn add_job(&mut self, start: Duration, job: Box<Job>) {
        let start = self.now + start;

        match self.timers.get_mut(&start) {
            Some(timer) => {
                timer.push_back(job);
            },
            None => {
                let mut timer = LinkedList::new();
                timer.push_back(job);
                self.timers.insert(start, timer);
            }
        }
        self.updated = true;
    }

    #[inline(always)]
    fn pop_jobs(&mut self) -> Option<LinkedList<Box<Job>>> {
        self.timers.pop_first().map(|(_,v)| v)
    }

    #[inline(always)]
    fn sync_time(&mut self) {
        self.now = Instant::now();
    }

    pub(crate) fn cancel(&mut self) {
        self.timers.clear();
    }
}

pub(crate) async fn run_jobs(s: Arc<Mutex<Scheduler>>) {
    lock!(s).sync_time();

    let mut timer = match lock!(s).pop_jobs() {
        Some(timer) => timer,
        None => return
    };

    while let Some(mut job) = timer.pop_front() {
        _ = job.invoke().await;
        let next_start = match job.interval {
            Some(interval) => interval,
            None => continue,
        };
        lock!(s).add_job(next_start, job);
    }
}
