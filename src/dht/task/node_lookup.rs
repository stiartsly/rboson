use std::sync::{Arc, Mutex};
use log::{debug, error};

use crate::{
    Id,
    NodeInfo,
};
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    rpc::{
        rpccall::RpcCall,
        node_entry::NodeEntry,
    },
    routing::{
        kbucket::KBucket,
        kclosest_nodes::KClosestNodes,
        kbucket_entry::KBucketEntry,
    },
    msg::{
        msg::{Body, Message},
        lookup_rsp::LookupResponse,
    },
    task::{
        task::{Task, TaskData, TaskResult},
        lookup_task::{LookupTask, LookupTaskData},
    }
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

    pub(crate) fn with_bootstrap(&mut self, bootstrap: bool) -> &mut Self {
        self.bootstrap = bootstrap;
        self
    }

    pub(crate) fn with_want_token(&mut self, token: bool) -> &mut Self {
        self.want_token = token;
        self
    }

    pub(crate) fn with_want_target(&mut self, want_target: bool) -> &mut Self {
        self.want_target = want_target;
        self
    }

    pub(crate) fn with_inject_candidates(&mut self, nodes: Vec<NodeInfo>) -> &mut Self {
        self.add_candidates_with_nodes(nodes);
        self
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

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn result(&self) -> Option<TaskResult> {
        self.result.as_ref().map(|v| TaskResult::NodeInfo(v.clone()))
    }

    fn prepare(&mut self) {
        let target = match self.bootstrap {
            true => self.target().distance(&Id::MAX_ID),
            false => self.target().clone()
        };

        let kes:Vec<KBucketEntry> = {
            let dht = Task::dht(self);
            let locked_dht = dht.lock().unwrap();
            let rt = locked_dht.rt();

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

            let ne = NodeEntry::from_candidate(next.clone());
            let network = Task::dht(self).lock().unwrap().network();
            let msg = Message::find_node_req(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.want_token
            );

            let msg = Arc::new(Mutex::new(msg));
            let handler = Consumer::new(move || {
                next.lock().unwrap().set_sent();
            });
            let _ = self.send_call(ne, msg, Some(handler)).map_err(|e| {
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

        let Some(msg) = call.rsp_ref() else {
            return;
        };
        let Some(Body::FindNodeRsp(body)) = msg.body() else {
            return;
        };

        let network = Task::dht(self).lock().unwrap().network();
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
        if self.result.is_some() {
            if LookupTask::done_on_eligible_result(self) {
                LookupTask::mark_lookup_done(self);
            }
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
