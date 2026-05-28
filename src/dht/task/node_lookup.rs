use std::{
    any::Any,
    sync::{Arc, Mutex}
};
use log::{debug, error};

use crate::{Id, NodeInfo};
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    rpc::{RpcCall, Target},
    msg::{Message, Body, LookupResponse},
    routing::{
        KBucket,
        KBucketEntry,
        KClosestNodes,
    },
    task::{
        Task, TaskData,
        LookupTask, LookupTaskData
    },
};

pub(crate) struct NodeLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    // Whether this is a bootstrap lookup, starting from nodes farthest
    // from the local node.
    bootstrap: bool,
	// Whether to request tokens in FIND_NODE RPCs for subsequent operations.
    want_token: bool,
    // Whether the task should filter the target node during the lookup process.
    want_target: bool,

    result: Option<NodeInfo>,
    dht: Arc<Mutex<DHT>>,
}

impl NodeLookupTask {
    pub(crate) fn new(
        dht: Arc<Mutex<DHT>>,
        target: Id,
        done_on_eligible_result: bool
    ) -> Self {
        Self {
            base_data: TaskData::new(),
            lookup_data: LookupTaskData::new(target, done_on_eligible_result),
            bootstrap: false,
            want_token: false,
            want_target: false,
            result: None,

            dht,
        }
    }

    pub(crate) fn with_bootstrap(&mut self, bootstrap: bool) {
        self.bootstrap = bootstrap;
    }

    pub(crate) fn with_want_token(&mut self, token: bool) {
        self.want_token = token;
    }

    pub(crate) fn with_want_target(&mut self, want_target: bool) {
        self.want_target = want_target;
    }

    pub(crate) fn with_inject_candidates(&mut self, nodes: Vec<NodeInfo>) {
        self.add_candidates_with_nodes(nodes);
    }

    pub(crate) fn result(&self) -> Option<NodeInfo> {
        self.result.clone()
    }

    #[cfg(test)]
    pub(crate) fn is_bootstrap(&self) -> bool {
        self.bootstrap
    }
    #[cfg(test)]
    pub(crate) fn want_token(&self) -> bool {
        self.want_token
    }
    #[cfg(test)]
    pub(crate) fn want_target(&self) -> bool {
        self.want_target
    }
}

impl LookupTask for NodeLookupTask {
    fn base_data(&self) -> &TaskData {
        &self.base_data
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn data(&self) -> &LookupTaskData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookupTaskData {
        &mut self.lookup_data
    }
}

impl Task for NodeLookupTask {
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn prepare(&mut self) {
        let target = match self.bootstrap {
            true => self.target().distance(&Id::MAX_ID),
            false => self.target().clone()
        };

        let kes:Vec<KBucketEntry> = {
            let rt = self.dht.lock().unwrap().rt();
            let mut kns = KClosestNodes::new(
                rt,
                target,
                KBucket::MAX_ENTRIES *3
            );
            kns.set_filter(|v| v.eligible_for_local_lookup());
            kns.fill();
            kns.into()
        };

        debug!("{}#{} initialized {} candidates for target {}",
            self.task_name(),
            self.task_id(),
            kes.len(),
            target,
        );
        self.add_candidates_with_kentries(kes);
    }

    fn iterate(&mut self) {
        LookupTask::iterate(self);

        while self.can_dorequest() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next,
                None => break
            };

            let target = Target::from_candidate(next.clone());
            let network = self.dht.lock().unwrap().network();
            let msg = Message::find_node_req(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.want_token
            );

            let handler = Consumer::new(move |_| {
                next.lock().unwrap().set_sent();
            });
            let _ = self.send_call(target, msg, Some(handler)).map_err(|e| {
                error!("Sending 'findNode' request error: {}", e);
            });
        }
    }

    fn call_sent(&mut self, _: &RpcCall) {}

    fn call_responded(&mut self, call: &RpcCall) {
        LookupTask::call_responded(self, call);

        if call.id_mismatched() {
            return;
        }

        let msg = call.rsp().expect("no response set");
        let Body::FindNodeRsp(body) = msg.body().expect("no message body") else {
            return;
        };

        let network = self.dht.lock().unwrap().network();
        let nodes = body.nodes(network);
        let Some(nodes) = nodes.filter(|v| !v.is_empty()) else {
            return;
        };

        self.add_candidates_with_nodes(nodes.to_vec());

        debug!("{}#{} adding {} candidates from response by target {}",
            self.task_name(),
            self.task_id(),
            nodes.len(),
            call.target_id()
        );

        if !self.want_target {
            return;
        }

        for item in nodes.iter() {
            if self.target() == item.id() {
                self.result = Some(item.clone());
                break;
            }
        }

        if self.result.is_none() {
            return;
        }

        // If the target node is found, consider the lookup done immediately.
        if LookupTask::data(self).done_on_eligible_result() {
            LookupTask::data_mut(self).mark_lookup_done();
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

unsafe impl Send for NodeLookupTask {}
unsafe impl Sync for NodeLookupTask {}
