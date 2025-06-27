use std::any::Any;
use std::rc::Rc;
use std::cell::RefCell;
use log::{warn, error};

use crate::{
    Id,
    Value,
    Network
};

use crate::dht::{
    constants,
    dht::DHT,
    rpccall::RpcCall,
    kclosest_nodes::KClosestNodes,
    msg::find_value_req as req,
    msg::find_value_rsp as rsp,
};

use crate::dht::msg::{
    msg::{Method, Kind, Msg},
    lookup_req::{Msg as LookupRequest},
    lookup_rsp::{Msg as LookupResponse},
};

use super::{
    task::{Task, TaskData},
    lookup_task::{LookupTask, LookupTaskData},
};

pub(crate) struct ValueLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    expected_seq: i32,
    result_fn: Box<dyn FnMut(Rc<RefCell<Box<dyn Task>>>, Option<Rc<Value>>)>,
    listeners: Vec<Box<dyn FnMut(&mut dyn Task)>>,
}

impl ValueLookupTask {
    pub(crate) fn new(dht: Rc<RefCell<DHT>>, target: Rc<Id>) -> Self {
        Self {
            base_data: TaskData::new(dht),
            lookup_data: LookupTaskData::new(target),
            expected_seq: -1,
            result_fn: Box::new(|_,_|{}),
            listeners: Vec::new(),
        }
    }

    pub(crate) fn set_result_fn<F>(&mut self, f: F)
    where F: FnMut(Rc<RefCell<Box<dyn Task>>>, Option<Rc<Value>>) + 'static,
    {
        self.result_fn = Box::new(f);
    }

    pub(crate) fn with_expected_seq(&mut self, seq: i32) {
        self.expected_seq = seq;
    }
}

impl LookupTask for ValueLookupTask {
    fn data(&self) -> &LookupTaskData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookupTaskData {
        &mut self.lookup_data
    }

    fn dht(&self) -> Rc<RefCell<DHT>> {
        Task::data(self).dht()
    }
}

impl Task for ValueLookupTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn add_listener(&mut self, cb: Box<dyn FnMut(&mut dyn Task)>) {
        self.listeners.push(cb);

    }
    fn notify_completion(&mut self) {
        while let Some(mut cb) = self.listeners.pop() {
            cb(self)
        }
    }

    fn prepare(&mut self) {
        let nodes = {
            let mut kns = KClosestNodes::with_filter(
                LookupTask::target(self),
                Task::data(self).dht().borrow().ni(),
                Task::data(self).dht().borrow().rt(),
                constants::MAX_ENTRIES_PER_BUCKET *2,
                move |_| true
            );
            kns.fill(false);
            kns.as_nodes()
        };
        self.add_candidates(&nodes);
    }

    fn update(&mut self) {
        while self.can_request() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next.clone(),
                None => break,
            };

            let msg = Rc::new(RefCell::new({
                let mut msg = Box::new(req::Message::new());
                msg.with_target(self.target());
                msg.with_want4(self.dht().borrow().network() == Network::IPv4);
                msg.with_want6(self.dht().borrow().network() == Network::IPv6);
                if self.expected_seq >= 0 {
                    msg.with_seq(self.expected_seq);
                }
                msg as Box<dyn Msg>
            }));

            let next = next.clone();
            let ni = next.borrow().ni();

            self.send_call(ni, msg, Box::new(move|_| {
                next.borrow_mut().set_sent();
            })).map_err(|e| {
               error!("Error on sending 'findValue' message: {}", e);
            }).ok();
        }
    }

    fn call_responsed(&mut self, call: &RpcCall, msg: Rc<RefCell<Box<dyn Msg>>>) {
        if !call.matches_id()||
            msg.borrow().kind() != Kind::Response ||
            msg.borrow().method() != Method::FindValue {
            return;
        }

        let borrowed = msg.borrow();
        let msg = match borrowed.as_any().downcast_ref::<rsp::Message>() {
            Some(v) => v,
            None => return
        };
        LookupTask::call_responsed(self, call, msg);

        if let Some(value) = msg.value() {
            let target = LookupTask::target(self);
            let id = value.id();

            if *target != id {
                warn!("Responsed value id {} mismatched with expected {}", id, target);
                return;
            }
            if !value.is_valid() {
                warn!("Responsed value {} is invalid, signature mismatch", id);
                return;
            }

            if self.expected_seq >=0 && value.sequence_number() < self.expected_seq {
                warn!("Responsed value {} is outdated, sequence {}, expected {}",
                    id, value.sequence_number(), self.expected_seq);
                return;
            }
        } else {
            let network = self.dht().borrow().network();
            let nodes = match network {
                Network::IPv4 => LookupResponse::nodes4(msg),
                Network::IPv6 => LookupResponse::nodes6(msg)
            };

            if let Some(nodes) = nodes {
                if !nodes.is_empty() {
                    self.add_candidates(nodes);
                }
            };
        }

        (self.result_fn)(self.base_data.task(), msg.value());
    }

    fn call_error(&mut self, call: &RpcCall) {
        LookupTask::call_error(self, call)
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        LookupTask::call_timeout(self, call)
    }
}
