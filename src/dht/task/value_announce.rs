use std::{
    any::Any,
    sync::{Arc, Weak, Mutex},
    collections::VecDeque,
};
use log::{debug, error};

use crate::Value;
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    msg::msg,
    task::{
        Task, TaskData,
        ClosestSet,
        CandidateNode,
    }
};

const MAX_TODO_ENTRIES: usize = 24;

pub(crate) struct ValueAnnounceTask {
    base_data: TaskData,

    todo: Arc<Mutex<VecDeque<Arc<Mutex<CandidateNode>>>>>,
    value: Value,
    expected_seq: i32,

    dht: Weak<Mutex<DHT>>
}

impl ValueAnnounceTask {
    pub(crate) fn new(
        dht: Weak<Mutex<DHT>>,
        value: Value,
        expected_seq: i32,
    ) -> Self {
        Self {
            base_data: TaskData::new(),
            todo: Arc::new(Mutex::new(
                VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            value,
            expected_seq,
            dht,
        }
    }

    pub(crate) fn with_closest(&self, closest: ClosestSet) -> &Self {
        let mut locked_todo = self.todo.lock().unwrap();
        let mut entries = closest.entries();

        while let Some(cn) = entries.pop() {
            if locked_todo.len() >= MAX_TODO_ENTRIES {
                break;
            }
            locked_todo.push_back(cn);
        }
        debug!(
            "{}#{} added {} nodes to announce queue",
            self.task_name(),
            self.task_id(),
            locked_todo.len()
        );
        self
    }
}

impl Task for ValueAnnounceTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn as_task(&self) -> &dyn Task {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dht(&self) -> Weak<Mutex<DHT>> {
        self.dht.clone()
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let cn = match self.todo.lock().unwrap().front() {
                Some(cn) => cn.clone(),
                None => break,
            };

            let token = cn.lock().unwrap().token();
            if token == 0 {
                self.todo.lock().unwrap().pop_front();
                continue;
            }
            let msg = msg::store_value_request(
                self.value.clone(),
                token,
                self.expected_seq,
            );

            let cloned_todo = self.todo.clone();
            let handler = Consumer::new(move |_| {
                cloned_todo.lock().unwrap().pop_front();
            });

            if let Err(e) = self.send_call(cn.into(), msg, Some(handler)) {
                error!("Sending 'storeValue' request error: {}", e);
            };
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() &&
            self.data().is_done()
    }
}

unsafe impl Send for ValueAnnounceTask {}
unsafe impl Sync for ValueAnnounceTask {}
