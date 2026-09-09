use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::dht::{
    dht::DHT,
    task::{
        task_listener::TaskListener,
        task_manager::TaskManager,
        Task,
        TaskData,
    },
};

struct PendingTask {
    data: TaskData,
    started: Rc<Cell<usize>>,
    handle: Rc<RefCell<Option<Rc<RefCell<Box<dyn Task>>>>>>,
}

impl PendingTask {
    fn new(
        started: Rc<Cell<usize>>,
        handle: Rc<RefCell<Option<Rc<RefCell<Box<dyn Task>>>>>>,
    ) -> Self {
        Self {
            data: TaskData::new(),
            started,
            handle,
        }
    }
}

impl Task for PendingTask {
    fn data(&self) -> &TaskData {
        &self.data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.data
    }

    fn as_task(&self) -> &dyn Task {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dht(&self) -> Rc<RefCell<DHT>> {
        unreachable!("TaskManager lifecycle test does not access DHT")
    }

    fn prepare(&mut self) {
        self.started.set(self.started.get() + 1);
        *self.handle.borrow_mut() = self.cloned().upgrade();
    }

    fn is_done(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_completion_starts_next_queued_task() {
        let manager = Rc::new(TaskManager::new());
        let started = Rc::new(Cell::new(0));
        let first_handle = Rc::new(RefCell::new(None));

        for index in 0..9 {
            let handle = if index == 0 {
                first_handle.clone()
            } else {
                Rc::new(RefCell::new(None))
            };
            manager.add(Box::new(PendingTask::new(started.clone(), handle)));
        }

        assert_eq!(started.get(), 8);

        let first = first_handle
            .borrow()
            .clone()
            .expect("first task should still be running");
        first.borrow_mut().complete();

        assert_eq!(started.get(), 9);
    }

    #[test]
    fn stop_cancels_running_task() {
        let manager = Rc::new(TaskManager::new());
        let started = Rc::new(Cell::new(0));
        let handle = Rc::new(RefCell::new(None));

        manager.add(Box::new(PendingTask::new(started.clone(), handle.clone())));
        assert_eq!(started.get(), 1);

        let task = handle.borrow().clone().expect("task should be running");
        manager.stop();

        assert!(task.borrow().is_canceled());
    }

    #[test]
    fn task_lifecycle_emits_matching_listener_event() {
        let manager = Rc::new(TaskManager::new());
        let started = Rc::new(Cell::new(0));
        let handle = Rc::new(RefCell::new(None));
        let completed = Rc::new(Cell::new(0));
        let canceled = Rc::new(Cell::new(0));
        let ended = Rc::new(Cell::new(0));

        manager.add(Box::new(PendingTask::new(started, handle.clone())));
        let task = handle.borrow().clone().expect("task should be running");
        task.borrow_mut().with_listener(
            TaskListener::default()
                .completed_fn({
                    let completed = completed.clone();
                    move |_| completed.set(completed.get() + 1)
                })
                .canceled_fn({
                    let canceled = canceled.clone();
                    move |_| canceled.set(canceled.get() + 1)
                })
                .ended_fn({
                    let ended = ended.clone();
                    move |_| ended.set(ended.get() + 1)
                }),
        );

        task.borrow_mut().complete();
        assert_eq!(completed.get(), 1);
        assert_eq!(canceled.get(), 0);
        assert_eq!(ended.get(), 1);

        manager.add(Box::new(PendingTask::new(Rc::new(Cell::new(0)), handle.clone())));
        let task = handle.borrow().clone().expect("task should be running");
        task.borrow_mut().with_listener(
            TaskListener::default()
                .completed_fn({
                    let completed = completed.clone();
                    move |_| completed.set(completed.get() + 1)
                })
                .canceled_fn({
                    let canceled = canceled.clone();
                    move |_| canceled.set(canceled.get() + 1)
                })
                .ended_fn({
                    let ended = ended.clone();
                    move |_| ended.set(ended.get() + 1)
                }),
        );

        task.borrow_mut().cancel();
        assert_eq!(completed.get(), 1);
        assert_eq!(canceled.get(), 1);
        assert_eq!(ended.get(), 2);
    }
}
