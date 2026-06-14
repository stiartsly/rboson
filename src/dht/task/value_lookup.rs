use std::{
    any::Any,
    sync::{Weak, Mutex}
};
use log::{debug, error, warn};

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

    result  : EligibleValue,
    dht     : Weak<Mutex<DHT>>,
}

impl ValueLookupTask {
    pub(crate) fn new(
        dht: Weak<Mutex<DHT>>,
        target: Id,
        expected_seq: i32,
        done_on_eligible_result: bool
    ) -> Self {
        Self {
            base_data   : TaskData::new(),
            lookup_data : LookupTaskData::new(target, done_on_eligible_result),
            result      : EligibleValue::new(target, expected_seq),
            dht         : dht.clone(),
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
        let entries:Vec<KBucketEntry> = {
            let rt = self.rt();
            let locked_rt = rt.lock().unwrap();
            let mut kns = KClosestNodes::new(
                &locked_rt,
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

        let network = self.network();
        while self.can_dorequest() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next.clone(),
                _ => break,
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
        let rsp  = call.rsp().expect("no response set.");
        let body = rsp.body().expect("no message body in response.");

        let Body::FindValueResponse(body) = body else {
            return;
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
            let nodes = body.nodes(self.network());

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
