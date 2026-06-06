use std::{
    any::Any,
    sync::{Mutex, Weak}
};
use log::{debug, error, warn, trace};

use crate::{Id, Value};
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    eligible_value::EligibleValue,
    rpc::RpcCall,
    msg::{msg, LookupResponse, Body},
    routing::{
        KBucket,
        KBucketEntry,
        KClosestNodes
    },
    task::{
        LookupTask, LookupTaskData,
        Task, TaskData,
    }
};

pub(crate) struct ValueLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    result: EligibleValue,
    dht: Weak<Mutex<DHT>>,
}

impl ValueLookupTask {
    pub(crate) fn new(
        dht: Weak<Mutex<DHT>>,
        target: Id,
        expected_seq: i32,
        done_on_eligible_result: bool
    ) -> Self {
        Self {
            base_data: TaskData::new(),
            lookup_data: LookupTaskData::new(target, done_on_eligible_result),
            result: EligibleValue::new(target, expected_seq),
            dht,
        }
    }

    pub(crate) fn result(&self) -> Option<Value> {
        self.result.value()
    }
}

impl LookupTask for ValueLookupTask {
    fn base_data(&self) -> &TaskData {
        &Task::data(self)
    }

    fn dht(&self) -> Weak<Mutex<DHT>> {
        Task::dht(self)
    }

    fn data(&self) -> &LookupTaskData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookupTaskData {
        &mut self.lookup_data
    }
}

impl Task for ValueLookupTask {
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

    fn prepare(&mut self) {
        let Some(strong_dht) = self.dht.upgrade() else {
            error!("{}#{} failed to prepare: DHT instance dropped",
                self.task_name(),
                self.task_id()
            );
            return;
        };

        let entries:Vec<KBucketEntry> = {
            let locked_dht = strong_dht.lock().unwrap();
            let mut kns = KClosestNodes::new(
                locked_dht.rt(),
                self.target().clone(),
                KBucket::MAX_ENTRIES *3
            );
            kns.set_filter(|v| v.eligible_for_local_lookup());
            kns.fill();
            kns.into()
        };

        debug!("{}#{} initialized {} candidates for target {}",
            self.task_name(),
            self.task_id(),
            entries.len(),
            self.target()
        );
        self.add(entries);
    }

    fn iterate(&mut self) {
        LookupTask::iterate(self);

        let dht = self.dht.upgrade()
            .expect("panic: DHT instance dropped.");
        let network = dht.lock().unwrap().network();

        trace!("{}#{} candidates.size={}",
            self.task_name(),
            self.task_id(),
            LookupTask::candidate_size(self)
        );

        while self.can_dorequest() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next.clone(),
                None => break,
            };

            let target = next.clone().into();
            let msg = msg::find_value_request(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.result.expected_seq(),
            );

            let cb = Consumer::new(move |_| {
                next.lock().unwrap().set_sent();
            });

            if let Err(e) = self.send_call(target, msg, Some(cb)) {
                error!("Sending 'find_value' request error: {}", e);
            };
        }
    }

    fn call_responded(&mut self, call: &RpcCall) {
        LookupTask::call_responded(self, call);

        if call.nodeid_mismatched() {
            return;
        }
        let rsp = call.rsp().expect("panic: should has response.");
        let body = rsp.body().expect("panic: should contain body in the response.");

        let Body::FindValueResponse(body) = body else {
            panic!("panic: should be findValue response body.");
        };

        if let Some(value) = body.value() {
            if !self.result.update(value.clone(), false) {
                warn!(
                    "{}#{} dropping value response from {} due to ineligible value data",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            }

            if self.result.is_empty() {
                return;
            }

            if LookupTask::data(self).done_on_eligible_result() {
                LookupTask::data_mut(self).done_lookup();
            }
        } else {
            let Some(strong_dht) = self.dht.upgrade() else {
                error!("{}#{} failed to iterate: DHT instance dropped",
                        self.task_name(),
                        self.task_id()
                );
                return;
            };

            let network = strong_dht.lock().unwrap().network();
            let nodes = body.nodes(network);
            drop(strong_dht);

            let Some(nodes) = nodes.filter(|v| !v.is_empty()) else {
                warn!("{}#{} received empty nodes list from {}, ignoring",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            };

            self.add(nodes.to_vec());

            debug!("{}#{} added {} additional candidates from response by target {}",
                self.task_name(),
                self.task_id(),
                nodes.len(),
                call.target_id()
            );
        }
    }

    fn call_error(&mut self, call: &RpcCall) {
        LookupTask::call_error(self, call);
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        LookupTask::call_timeout(self, call);
    }

    fn is_done(&self) -> bool {
        LookupTask::is_done(self)
    }
}

unsafe impl Send for ValueLookupTask {}
unsafe impl Sync for ValueLookupTask {}
