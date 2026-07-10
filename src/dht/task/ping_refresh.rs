use std::{
    any::Any,
    rc::Rc,
    cell::RefCell,
    collections::VecDeque,
};
use crate::dht::{
    dht::DHT,
    msg::msg,
    rpc::RpcCall,
    handler::Handler,
    task::{Task, TaskData},
    routing::{KBucket, KBucketEntry}
};

#[allow(unused)]
pub(crate) struct PingRefreshTask {
    base_data: TaskData,

    todo: Rc<RefCell<VecDeque<KBucketEntry>>>,
    // Whether to ping all nodes in the bucket, regardless of their ping status.
    check_all: bool,
	// Whether to remove nodes from the routing table if their PING RPC times out.
    remove_on_timeout: bool,

    dht: Rc<RefCell<DHT>>
}

const MAX_TODO_ENTRIES: usize = KBucket::MAX_ENTRIES * 2;

#[allow(unused)]
impl PingRefreshTask {
    pub(crate) fn new(dht: Rc<RefCell<DHT>>) -> Self {
        Self {
            base_data: TaskData::new(),
            todo: Rc::new(RefCell::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),

            check_all           : false,
            remove_on_timeout   : false,
            dht                 : dht.clone()
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

    pub(crate) fn with_bucket(&mut self, bucket: Rc<RefCell<KBucket>>) -> &mut Self {
        let mut borrowed_bucket = bucket.borrow_mut();
        let mut borrowed_todo   = self.todo.borrow_mut();

        borrowed_bucket.update_refresh_time();
        for item in borrowed_bucket.entries().iter() {
            if self.check_all || self.remove_on_timeout || item.needs_ping() {
                if borrowed_todo.len() >= MAX_TODO_ENTRIES {
                    break;
                }
                borrowed_todo.push_back(item.clone());
            }
        }
        drop(borrowed_bucket);
        drop(borrowed_todo);

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

    fn dht(&self) -> Rc<RefCell<DHT>> {
        self.dht.clone()
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        if !self.remove_on_timeout {
            log::debug!(
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
        log::debug!("{}#{} removing timeout entry {} from routing table",
            self.task_name(),
            self.task_id(),
            target_id
        );

        let rt = self.dht().borrow().rt();
        let mut borrowed_rt = rt.borrow_mut();
        borrowed_rt.remove(&target_id);
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let kentry = match self.todo.borrow().front() {
                Some(v) => v.clone(),
                _ => break,
            };

            if !self.check_all && !kentry.needs_ping() {
                _ = self.todo.borrow_mut().pop_front();
                continue;
            }

            let msg  = msg::ping_request();
            let todo = self.todo.clone();
            let cb = Handler::new(move |_| {
                todo.borrow_mut().pop_front();
            });

            self.send_call(kentry.into(), msg, Some(cb));
        }
    }

    fn is_done(&self) -> bool {
        self.todo.borrow().is_empty() &&
            self.data().is_done()
    }
}
