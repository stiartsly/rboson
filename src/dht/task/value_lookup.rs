use std::sync::{Arc, Mutex};
use log::{debug, error, warn};

use crate::{Id, Network};
use crate::dht::{
    dht::DHT,
    rpccall::RpcCall,
    node_entry::NodeEntry,
    eligible_value::EligibleValue,
    msg::{
        lookup_rsp::LookupResponse,
        msg::{Body, Message},
    },
    routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
        kclosest_nodes::KClosestNodes,
    },
    task::{
        lookup_task::{LookupTask, LookupTaskData},
        task::{Task, TaskData, TaskResult},
    },
};

pub(crate) struct ValueLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,
    expected_seq: i32,

    result: EligibleValue
}

impl ValueLookupTask {
    pub(crate) fn new(
        dht: Arc<Mutex<DHT>>,
        target: Id,
        expected_seq: i32,
        done_on_eligible_result: bool
    ) -> Self {
        Self {
            base_data: TaskData::new(dht),
            lookup_data: LookupTaskData::new(target.clone(), done_on_eligible_result),
            expected_seq,
            result: EligibleValue::new(target, expected_seq),
        }
    }
}

impl LookupTask for ValueLookupTask {
    fn base_data(&self) -> &TaskData {
        &Task::data(self)
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        Task::data(self).dht()
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

    fn result(&self) -> Option<TaskResult> {
        self.result.value().as_ref().map(|v| TaskResult::Value(v.clone()))
    }

    fn prepare(&mut self) {
        let entries:Vec<KBucketEntry> = {
            let mut kns = KClosestNodes::new(
                self.dht().lock().unwrap().rt(),
                self.target().clone(),
                KBucket::MAX_ENTRIES *3
            );
            kns.set_filter(|v| v.eligible_for_local_lookup());
            kns.fill();
            kns.into()
        };

        debug!("{}#{} initialized {} candidates for target {}",
            self.name(),
            self.id(),
            entries.len(),
            self.target()
        );
        self.add_candidates_with_kentries(entries);
    }

    fn iterate(&mut self) {
        LookupTask::iterate(self);

        while self.can_dorequest() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next.clone(),
                None => break,
            };

            let entry = NodeEntry::from_candidate(next.clone());
            let network = self.dht().lock().unwrap().network();
            let msg = Arc::new(Mutex::new(Message::find_value_req(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.expected_seq,
            )));

            _ = self.send_call(entry, msg, Box::new(move |_| {
                next.lock().unwrap().set_sent();
            })).map_err(|e| {
                error!("Error on sending 'find_value' request: {}", e);
            });
        }
    }

    fn call_responsed(&mut self, call: &RpcCall) {
        LookupTask::call_responsed(self, call);

        if call.id_mismatched() {
            return;
        }

        let msg = call.rsp().unwrap();
        let locked_msg = msg.lock().unwrap();
        let Some(Body::FindValueRsp(body)) = locked_msg.body() else {
            warn!("{}#{} ignoring non LookupValue response from {}",
                self.name(),
                self.id(),
                call.target_id()
            );
            return;
        };

        if let Some(value) = body.value() {
            if !self.result.update(value.clone(), true) {
                warn!(
                    "{}#{} dropping value response from {} due to ineligible value data",
                    self.name(),
                    self.id(),
                    call.target_id()
                );
                return;
            }
            if !self.result.is_empty() {
                if LookupTask::done_on_eligible_result(self) {
                    debug!("{}#{} value is eligible, mark lookup done",
                        self.name(),
                        self.id()
                    );
                    LookupTask::mark_lookup_done(self);
                } else {
                    debug!("{}#{} value is ineligible, continue iteration for more precise results",
                        self.name(),
                        self.id()
                    );
                }
            }
        } else {
            let network = self.dht().lock().unwrap().network();
            let nodes = match network {
                Network::IPv4 => body.nodes4(),
                Network::IPv6 => body.nodes6()
            };

            let Some(nodes) = nodes.filter(|v|v.is_empty()) else {
                warn!("{}#{} received empty nodes list from {}, ignoring",
                    self.name(),
                    self.id(),
                    call.target_id()
                );
                return;
            };

            self.add_candidates_with_nodes(nodes.to_vec());

            debug!("{}#{} added {} additional candidates from {} for target {}",
                self.name(),
                self.id(),
                nodes.len(),
                call.target_id(),
                self.target()
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
