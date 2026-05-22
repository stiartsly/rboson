use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use log::{debug, error};

use crate::dht::{
    dht::DHT,
    node_entry::NodeEntry,
    rpccall::RpcCall,
    msg::msg::Message,
    task::task::{Task, TaskData},
    routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
    }
};

pub(crate) struct PingRefreshTask {
    base_data: TaskData,

    todo: Arc<Mutex<VecDeque<KBucketEntry>>>,

    // Whether to ping all nodes in the bucket, regardless of their ping status.
    check_all: bool,

	// Whether to remove nodes from the routing table if their PING RPC times out.
    remove_on_timeout: bool,
}

const MAX_TODO_ENTRIES: usize = KBucket::MAX_ENTRIES * 2;

impl PingRefreshTask {
    pub(crate) fn new(dht: Arc<Mutex<DHT>>) -> Self {
        Self {
            base_data: TaskData::new(dht),
            todo: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),

            check_all           : false,
            remove_on_timeout   : false,
        }
    }

    pub(crate) fn with_check_all(&mut self, check_all: bool) -> &mut Self {
        self.check_all = check_all;
        self
    }

    pub(crate) fn with_remove_on_timeout(&mut self, remove_on_timeout: bool) -> &mut Self {
        self.remove_on_timeout = remove_on_timeout;
        self
    }

    pub(crate) fn with_bucket(&mut self, bucket: Arc<Mutex<KBucket>>) -> &mut Self {
        let mut entries = {
            let mut locked_bucket = bucket.lock().unwrap();
            locked_bucket.update_refresh_time();
            locked_bucket.entries()
        };

        let mut todo = self.todo.lock().unwrap();
        while let Some(entry) = entries.pop() {
            if self.check_all || self.remove_on_timeout || entry.needs_ping() {
                if todo.len() >= MAX_TODO_ENTRIES {
                    break;
                }
                todo.push_back(entry);
            }
        }
        drop(todo);
        self
    }
}

impl Task for PingRefreshTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        if !self.remove_on_timeout {
            debug!(
                "{}#{} timeout for node {}, not removed (remove_on_timeout=false)",
                self.name(),
                self.id(),
                call.target_id()
            );
            return;
        }

        let target_id = call.target_id();
        // CAUSION:
        // Should not use the original bucket object,
        // because the routing table is dynamic, maybe already changed.
        debug!("{}#{} removing timeout entry {} from routing table",
            self.name(),
            self.id(),
            target_id
        );

        let rt = self.base_data.dht().lock().unwrap().rt();
        rt.lock().unwrap().remove(&target_id);
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let kentry = match self.todo.lock().unwrap().front() {
                Some(v) => v.clone(),
                None => break,
            };

            if !self.check_all && !kentry.needs_ping() {
                _ = self.todo.lock().unwrap().pop_front();
                continue;
            }

            let msg = Arc::new(Mutex::new(Message::ping_req()));
            let todo = self.todo.clone();
            let ke = NodeEntry::from_kentry(kentry);

            self.send_call(ke, msg, Box::new(move|_| {
                todo.lock().unwrap().pop_front();
            })).map_err(|e| {
               error!("Error on sending 'PingRequest' message: {}", e);
            }).ok();
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() && Task::is_done(self)
    }
}

unsafe impl Send for PingRefreshTask {}
unsafe impl Sync for PingRefreshTask {}
