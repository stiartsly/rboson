use std::{
    any::Any,
    sync::{Arc, Weak, Mutex},
    collections::VecDeque,
};
use log::{debug, error};

use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    msg::msg,
    task::{Task, TaskData},
    rpc::RpcCall,
    routing::{
        KBucket, KBucketEntry,
        RoutingTable
    }
};

#[allow(unused)]
pub(crate) struct PingRefreshTask {
    base_data: TaskData,

    todo: Arc<Mutex<VecDeque<KBucketEntry>>>,
    // Whether to ping all nodes in the bucket, regardless of their ping status.
    check_all: bool,
	// Whether to remove nodes from the routing table if their PING RPC times out.
    remove_on_timeout: bool,

    dht: Weak<Mutex<DHT>>,
    rt : Arc<Mutex<RoutingTable>>,
}

const MAX_TODO_ENTRIES: usize = KBucket::MAX_ENTRIES * 2;

#[allow(unused)]
impl PingRefreshTask {
    pub(crate) fn new(dht: Weak<Mutex<DHT>>) -> Self {
        let strong = dht.upgrade().expect("DHT instance dropped");
        let locked = strong.lock().unwrap();

        Self {
            base_data: TaskData::new(),
            todo: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),

            check_all           : false,
            remove_on_timeout   : false,
            dht                 : dht.clone(),
            rt                  : locked.rt(),
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
        let mut bucket = bucket.lock().unwrap();
        let mut todo   = self.todo.lock().unwrap();

        bucket.update_refresh_time();
        for item in bucket.entries().iter() {
            if self.check_all || self.remove_on_timeout || item.needs_ping() {
                if todo.len() >= MAX_TODO_ENTRIES {
                    break;
                }
                todo.push_back(item.clone());
            }
        }
        drop(bucket);
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

    fn as_task(&self) -> &dyn Task {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dht(&self) -> Weak<Mutex<DHT>> {
        self.dht.clone()
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        if !self.remove_on_timeout {
            debug!(
                "{}#{} timeout for node {}, not removed (remove_on_timeout=false)",
                self.task_name(),
                self.task_id(),
                call.target_id()
            );
            return;
        }

        let target_id = call.target_id();
        // CAUSION:
        // Should not use the original bucket object,
        // because the routing table is dynamic, maybe already changed.
        debug!("{}#{} removing timeout entry {} from routing table",
            self.task_name(),
            self.task_id(),
            target_id
        );

        let mut rt = self.rt.lock().unwrap();
        rt.remove(&target_id);
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

            let msg  = msg::ping_request();
            let todo = self.todo.clone();
            let cb = Consumer::new(move |_| {
                todo.lock().unwrap().pop_front();
            });
            if let Err(e) = self.send_call(kentry.into(), msg, Some(cb)) {
               error!("Error on sending 'PingRequest' message: {}", e);
            }
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() &&
            self.data().is_done()
    }
}

unsafe impl Send for PingRefreshTask {}
unsafe impl Sync for PingRefreshTask {}
