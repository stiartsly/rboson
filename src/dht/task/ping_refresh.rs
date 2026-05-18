use std::collections::LinkedList;
use std::sync::{Arc, Mutex};
use log::{error, debug};

use crate::dht::{
    node_entry::NodeEntry,
    rpccall::RpcCall,
    dht::DHT,
    msg::msg::Message,
    task::task::{Task, TaskData},
    routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
    }
};

pub(crate) struct PingRefreshTask {
    base_data: TaskData,

    todo: Arc<Mutex<LinkedList<KBucketEntry>>>,

    // Whether to ping all nodes in the bucket, regardless of their ping status.
    check_all: bool,

	// Whether to remove nodes from the routing table if their PING RPC times out.
    remove_on_timeout: bool,

	// Whether to ping a replacement node from the bucket’s cache.
    probe_replacement: bool,
}

impl PingRefreshTask {
    pub(crate) fn new(dht: Arc<Mutex<DHT>>) -> Self {
        Self {
            base_data: TaskData::new(dht),
            todo: Arc::new(Mutex::new(LinkedList::new())),

            check_all           : false,
            probe_replacement   : false,
            remove_on_timeout   : false,
        }
    }

    pub(crate) fn with_check_all(&mut self, check_all: bool) -> &mut Self {
        self.check_all = check_all;
        self
    }

    pub(crate) fn remove_on_timeout(&mut self, remove_on_timeout: bool) -> &mut Self {
        self.remove_on_timeout = remove_on_timeout;
        self
    }

    pub(crate) fn with_probe_replacement(&mut self, replacement: bool) -> &mut Self {
        self.probe_replacement = replacement;
        self
    }

    pub(crate) fn bucket(&mut self, bucket: Arc<Mutex<KBucket>>) -> &mut Self {
        bucket.lock().unwrap().update_refresh_time();

        let mut todo = self.todo.lock().unwrap();
        let mut entries = bucket.lock().unwrap().entries();
        while let Some(entry) = entries.pop() {
            if self.check_all || self.remove_on_timeout || entry.needs_ping() {
                if todo.len() >= KBucket::MAX_ENTRIES*2 {
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
            }

            let msg = Arc::new(Mutex::new(Message::ping_req()));
            let todo = self.todo.clone();
            let ke = NodeEntry::from_kentry(kentry);

            self.send_call(ke, msg, Box::new(move|_| {
                todo.lock().unwrap().pop_front();
            })).map_err(|e| {
               error!("Error on sending 'pingRequest' message: {}", e);
            }).ok();
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() && Task::is_done(self)
    }
}

unsafe impl Send for PingRefreshTask {}
unsafe impl Sync for PingRefreshTask {}
