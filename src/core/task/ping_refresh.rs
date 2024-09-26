use std::any::Any;
use std::collections::LinkedList;
use std::rc::Rc;
use std::cell::RefCell;
use log::{error, debug};

use crate::core::{
    kbucket::KBucket,
    kbucket_entry::KBucketEntry,
    rpccall::RpcCall,
    dht::DHT,
};

use crate::core::msg::{
    ping_req::Message,
    msg::Msg,
};

use super::task::{
    Task,
    TaskData
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PingOption {
    CheckAll = 0,
    RemoveOnTimeout = 1,
    ProbeCache = 3
}

pub(crate) struct PingRefreshTask {
    base_data: TaskData,

    bucket: Rc<RefCell<KBucket>>,
    todo: Rc<RefCell<LinkedList<Rc<RefCell<KBucketEntry>>>>>,

    check_all: bool,
    _probe_cache: bool,
    remove_on_timeout: bool,
}

impl PingRefreshTask {
    pub(crate) fn new(
        dht: Rc<RefCell<DHT>>,
        bucket: Rc<RefCell<KBucket>>,
        option: PingOption
    ) -> Self {
        let mut task = Self {
            base_data: TaskData::new(dht),
            bucket: bucket,
            todo: Rc::new(RefCell::new(LinkedList::new())),

            check_all:          option == PingOption::CheckAll,
            _probe_cache:       option == PingOption::ProbeCache,
            remove_on_timeout:  option == PingOption::RemoveOnTimeout
        };
        task.sync_from_bucket();
        task
    }

    fn sync_from_bucket(&mut self) {
        self.bucket.borrow_mut().update_refresh_time();
        self.bucket.borrow().entries().iter().for_each(|entry| {
            if entry.borrow().needs_ping() || self.check_all || self.remove_on_timeout {
                self.todo.borrow_mut().push_back(entry.clone())
            }
        })
    }
}

impl Task for PingRefreshTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn update(&mut self) {
        while self.can_request() {
            let cn = match self.todo.borrow().front() {
                Some(cn) => cn.clone(),
                None => break,
            };

            if !self.check_all && !cn.borrow().needs_ping() {
                self.todo.borrow_mut().pop_front();
            }

            let msg  = Rc::new(RefCell::new(
                Box::new(Message::new()) as Box<dyn Msg>
            ));
            let todo = self.todo.clone();
            let ni = cn.borrow().ni();

            self.send_call(ni, msg, Box::new(move|_| {
                todo.borrow_mut().pop_front();
            })).map_err(|e| {
               error!("Error on sending 'pingRequest' message: {}", e);
            }).ok();
        }
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        if self.remove_on_timeout {
            return;
        }

        // CAUSION:
        // Should not use the original bucket object,
        // because the routing table is dynamic, maybe already changed.
        debug!("Removing invalid entry from routing table");

        Task::data(self).dht()
            .borrow().rt()
            .borrow_mut()
            .remove(call.target_id());
    }
}
