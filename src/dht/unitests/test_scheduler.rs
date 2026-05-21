use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};

use crate::dht::scheduler::Scheduler;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_oneshot_task() {
        let sched = Scheduler::new();
        let (tx, mut rx) = mpsc::unbounded_channel();

        sched.add(Duration::from_millis(20), None, move || {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send("oneshot");
            })
        })
        .unwrap();

        let value = timeout(Duration::from_millis(200), rx.recv()).await.unwrap();
        assert_eq!(value, Some("oneshot"));

        sched.stop().await.unwrap();
    }

    #[tokio::test]
    async fn cancel_prevents_future_execution() {
        let sched = Scheduler::new();
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_task = fired.clone();

        let handle = sched.add(Duration::from_millis(80), None, move || {
                let fired_task = fired_task.clone();
                Box::pin(async move {
                    fired_task.fetch_add(1, Ordering::Relaxed);
                })
            })
        .unwrap();

        assert!(sched.cancel(handle).await.unwrap());

        sleep(Duration::from_millis(140)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);

        sched.stop().await.unwrap();
    }

    #[tokio::test]
    async fn add_while_waiting_reschedules_to_earlier_deadline() {
        let sched = Scheduler::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let late_tx = tx.clone();

        sched.add(Duration::from_millis(150), None, move || {
            let tx = late_tx.clone();
            Box::pin(async move {
                let _ = tx.send("late");
            })
        })
        .unwrap();

        sleep(Duration::from_millis(20)).await;

        let early_tx = tx.clone();

        sched.add(Duration::from_millis(15), None, move || {
            let tx = early_tx.clone();
            Box::pin(async move {
                let _ = tx.send("early");
            })
        })
        .unwrap();

        let first = timeout(Duration::from_millis(120), rx.recv()).await.unwrap();
        assert_eq!(first, Some("early"));

        let second = timeout(Duration::from_millis(200), rx.recv()).await.unwrap();
        assert_eq!(second, Some("late"));

        sched.stop().await.unwrap();
    }

    #[tokio::test]
    async fn periodic_task_runs_multiple_times_and_can_be_canceled() {
        let sched = Scheduler::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runs = Arc::new(AtomicUsize::new(0));
        let runs_task = runs.clone();

    let handle = sched.add(Duration::from_millis(10), Some(Duration::from_millis(200)), move || {
            let tx = tx.clone();
            let runs_task = runs_task.clone();
            Box::pin(async move {
                let count = runs_task.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = tx.send(count);
            })
        })
        .unwrap();

        let first = timeout(Duration::from_millis(100), rx.recv()).await.unwrap();
        assert_eq!(first, Some(1));

        let second = timeout(Duration::from_millis(260), rx.recv()).await.unwrap();
        assert_eq!(second, Some(2));

        assert!(sched.cancel(handle).await.unwrap());

        let runs_after_cancel = runs.load(Ordering::Relaxed);
        sleep(Duration::from_millis(260)).await;
        assert_eq!(runs.load(Ordering::Relaxed), runs_after_cancel);

        sched.stop().await.unwrap();
    }
}
