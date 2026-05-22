use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use log::{debug, error, warn};

use crate::Value;
use crate::dht::{
    dht::DHT,
    msg::msg::Message,
    node_entry::NodeEntry,
};

use super::{
    task::{Task, TaskData, TaskResult},
    closest_set::ClosestSet,
    candidate_node::CandidateNode,
};

pub(crate) struct ValueAnnounceTask {
    base_data: TaskData,

    todo: Arc<Mutex<VecDeque<Arc<Mutex<CandidateNode>>>>>,
    value: Value,
    expected_seq: i32,
}

const MAX_TODO_ENTRIES: usize = 24;

impl ValueAnnounceTask {
    pub(crate) fn new(
        dht: Arc<Mutex<DHT>>,
        value: Value,
        expected_seq: i32,
    ) -> Self {
        Self {
            base_data: TaskData::new(dht),
            todo: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            value,
            expected_seq,
        }
    }

    pub(crate) fn with_closest(&mut self, closest: ClosestSet) -> &mut Self {
        let mut todo = self.todo.lock().unwrap();
        let mut entries = closest.entries();
        while let Some(cn) = entries.pop() {
            if todo.len() >= MAX_TODO_ENTRIES {
                break;
            }
            todo.push_back(cn);
        }
        debug!(
            "{}#{} added {} nodes to announce queue",
            self.name(),
            self.id(),
            todo.len()
        );
        drop(todo);
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

    fn result(&self) -> Option<TaskResult> {
        None
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

            let msg = Arc::new(Mutex::new(Message::store_value_req(
                self.value.clone(),
                token,
                self.expected_seq,
            )));

            let todo = self.todo.clone();
            let entry = NodeEntry::from_candidate(cn);

            self.send_call(entry, msg, Box::new(move|_| {
                todo.lock().unwrap().pop_front();
            })).map_err(|e| {
               error!("Error on sending 'storeValue' message: {}", e);
            }).ok();

            break;
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() && self.data().is_done()
    }
}

unsafe impl Send for ValueAnnounceTask {}
unsafe impl Sync for ValueAnnounceTask {}
