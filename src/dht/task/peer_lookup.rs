use std::{
    any::Any,
    rc::Rc,
    cell::RefCell
};
use crate::{Id, PeerInfo, EasyHandler};
use crate::dht::{
    dht::DHT,
    eligible_peers::EligiblePeers,
    rpc::RpcCall,
    msg::{msg, Body, LookupResponse},
    task::{
        Task, TaskData,
        LookupTask, LookupTaskData,
    },
};

pub(crate) struct PeerLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    result: EligiblePeers,
    dht: Rc<RefCell<DHT>>
}

impl PeerLookupTask {
    pub(crate) fn new(
        dht: Rc<RefCell<DHT>>,
        target: Id,
        expected_seq: i32,
        expected_count: usize,
        done_on_eligible_result: bool
    ) -> Self {
        Self {
            base_data   : TaskData::new(),
            lookup_data : LookupTaskData::new(target, done_on_eligible_result),
            result      : EligiblePeers::new(target, expected_seq, expected_count),
            dht         : dht.clone()
        }
    }

    pub(crate) fn result(&self) -> Vec<PeerInfo> {
        self.result.peers()
    }
}

impl LookupTask for PeerLookupTask {
    fn base_data(&self) -> &TaskData {
        &self.base_data
    }

    fn lookup_data(&self) -> &LookupTaskData {
        &self.lookup_data
    }

    fn lookup_data_mut(&mut self) -> &mut LookupTaskData {
        &mut self.lookup_data
    }
}

impl Task for PeerLookupTask {
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

    fn prepare(&mut self) {
        self.seed_candidates(self.target().clone());
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
            let msg = msg::find_peer_request(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.result.expected_seq(),
                self.result.expected_count() as i32,
            );

            let cb = EasyHandler::new(move |_| {
                next.borrow_mut().set_sent();
            });
            self.send_call(target, msg, Some(cb));
        }
    }

    fn call_responded(&mut self, call: &RpcCall) {
        LookupTask::call_responded(self, call);

        if call.nodeid_mismatched() {
            return;
        }

        let rsp  = call.rsp().expect("no response set.");
        let body = rsp.body().expect("no message body in response.");
        let Body::FindPeerResponse(body) = body else {
            return;
        };

        if let Some(peers) = body.peers() {
            if peers.is_empty() {
                log::warn!("{}#{} received empty peers from {}, ignoring",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            }

            if !self.result.add(peers.to_vec(), false) {
                log::warn!(
                    "{}#{} dropping peer response from {} due to ineligible peer data",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            }

            log::debug!("{}#{} received {} peers from response by {}",
                self.task_name(),
                self.task_id(),
                peers.len(),
                call.target_id()
            );

            if self.result.reached_capacity() {
                if LookupTask::lookup_data(self).done_on_eligible_result() {
                    LookupTask::lookup_data_mut(self).done_lookup();
                }
                self.result.prune();
            }
        } else {
            let nodes = body.nodes(self.network());
            let Some(nodes) = nodes.filter(|v| !v.is_empty()) else {
                log::warn!("{}#{} received empty nodes list from {}, ignoring",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            };

            self.add(nodes.to_vec());

            log::debug!("{}#{} added {} candidates from response by {}",
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
